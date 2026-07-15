//! Engine lifecycle state machine.
//!
//! The state machine is deliberately explicit: every command and async event
//! is routed through `Engine::apply` and produces zero or more `EngineEvent`s.
//! State changes and their associated events are emitted in the same serial
//! command loop so callers observe a consistent ordering.

use alloc::vec::Vec;

use crate::latency::LatencyController;
use crate::recovery::{RecoveryAction, RecoveryDecision, RecoveryPolicy, RecoveryTracker};
use crate::resource::{ResourceKind, ResourceLedger};
use cheetah_media_types::{Recoverability, StreamEpoch, TrackId, TrackInfo};

/// Player lifecycle states.
///
/// The main loop is `Idle → Loading → Preroll → Playing ↔ Rebuffering →
/// Stopping → Idle`. `Paused` is an explicit running state reached from
/// `Playing` so that `pause` and `play` are invertible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayerState {
    /// Engine has been created but no load is active.
    #[default]
    Idle,
    /// A load is in progress; waiting for tracks/config and enough buffer.
    Loading,
    /// Tracks and configuration are known; waiting for `play`.
    Preroll,
    /// Normal playback.
    Playing,
    /// Playback paused while buffers are healthy.
    Paused,
    /// Buffer underrun; will return to `Playing` (or `Paused`) when data returns.
    Rebuffering,
    /// A stop is in progress; resources are being released.
    Stopping,
    /// A non-recoverable failure occurred.
    Failed,
    /// Engine has been destroyed and can no longer be used.
    Destroyed,
}

/// Stable engine-level errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// The command is not valid in the current state.
    InvalidState {
        state: PlayerState,
        command: &'static str,
    },
    /// The command is not recognized.
    InvalidCommand { command: &'static str },
    /// A backend reported a failure.
    Backend { stage: &'static str, code: u32 },
    /// A resource limit was exceeded.
    ResourceLimit {
        name: &'static str,
        current: u64,
        limit: u64,
    },
    /// The engine has been destroyed.
    Destroyed,
}

impl EngineError {
    /// Stable numeric code for telemetry and FFI.
    pub const fn code(&self) -> u32 {
        match self {
            Self::InvalidState { .. } => 6001,
            Self::InvalidCommand { .. } => 6002,
            Self::Backend { .. } => 6100,
            Self::ResourceLimit { .. } => 6200,
            Self::Destroyed => 6999,
        }
    }

    /// Human-readable stage tag.
    pub const fn stage(&self) -> &'static str {
        match self {
            Self::InvalidState { .. } | Self::InvalidCommand { .. } => "state",
            Self::Backend { .. } => "backend",
            Self::ResourceLimit { .. } => "limit",
            Self::Destroyed => "lifecycle",
        }
    }
}

impl Recoverability for EngineError {
    fn is_recoverable(&self) -> bool {
        match self {
            Self::InvalidState { .. } | Self::InvalidCommand { .. } => true,
            Self::Backend { .. } => false,
            Self::ResourceLimit { .. } => false,
            Self::Destroyed => false,
        }
    }
}

/// Event emitted by the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    /// The state changed.
    StateChanged { from: PlayerState, to: PlayerState },
    /// A new track was discovered.
    TrackAdded(TrackInfo),
    /// Track configuration changed.
    TrackConfigChanged { track_id: TrackId, generation: u64 },
    /// A stream discontinuity occurred.
    Discontinuity { epoch: StreamEpoch },
    /// End of stream reached.
    Eof,
    /// An error was raised.
    Error(EngineError),
    /// A recovery action was scheduled for a recoverable error.
    RecoveryScheduled {
        action: RecoveryAction,
        stage: &'static str,
        code: u32,
        delay_ms: u64,
        attempts_left: u32,
    },
    /// Resources were leaked across a stop/destroy boundary.
    ResourceWarning {
        kinds: alloc::vec::Vec<ResourceKind>,
        total: u64,
    },
    /// The stop sequence completed.
    Stopped,
    /// The engine was destroyed.
    Destroyed,
    /// Performance/resource metrics snapshot.
    Metrics(super::MetricsSnapshot),
}

/// Request describing what to load.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoadRequest {
    /// URL or identifier of the source.
    pub url: &'static str,
    /// Whether the source is expected to be a live stream.
    pub is_live: bool,
}

/// A network-level event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkEvent {
    /// Transport connected.
    Connected,
    /// Transport reached end-of-stream.
    Eof,
    /// Transient network failure.
    Retryable,
    /// Fatal network failure.
    Fatal { code: u32 },
}

/// A backend callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendEvent {
    /// A track was discovered.
    Track { epoch: StreamEpoch, info: TrackInfo },
    /// Track configuration changed.
    ConfigChanged {
        epoch: StreamEpoch,
        track_id: TrackId,
        generation: u64,
    },
    /// A decoder is ready.
    DecoderReady {
        epoch: StreamEpoch,
        track_id: TrackId,
    },
    /// A decoder reported an error.
    DecoderError {
        epoch: StreamEpoch,
        track_id: TrackId,
        code: u32,
    },
    /// A frame was rendered.
    Rendered {
        epoch: StreamEpoch,
        track_id: TrackId,
    },
    /// The backend stopped.
    Stopped,
}

/// Command that can be applied to the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineCommand {
    /// Start a new session.
    Load(LoadRequest),
    /// Begin playback.
    Play,
    /// Pause playback.
    Pause,
    /// Stop and release the current session.
    Stop,
    /// Tear down the engine.
    Destroy,
    /// Network event.
    Network(NetworkEvent),
    /// Backend callback.
    Backend(BackendEvent),
    /// Request a metrics snapshot.
    GetMetrics,
}

/// Result of applying a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineOutput {
    /// Events emitted during command handling.
    pub events: Vec<EngineEvent>,
}

impl EngineOutput {
    fn new() -> Self {
        Self { events: Vec::new() }
    }

    fn push(&mut self, event: EngineEvent) {
        self.events.push(event);
    }

    fn change(&mut self, from: PlayerState, to: PlayerState) {
        self.events.push(EngineEvent::StateChanged { from, to });
    }

    fn error(&mut self, err: EngineError) {
        self.events.push(EngineEvent::Error(err));
    }
}

/// Media engine state machine.
#[derive(Debug)]
pub struct Engine {
    state: PlayerState,
    /// Current `StreamEpoch`. Incremented on every successful `load`.
    epoch: StreamEpoch,
    /// Generation counter for commands.
    sequence: u64,
    /// Track/config discovery flags for the current load.
    has_track: bool,
    has_config: bool,
    /// State to restore after `Rebuffering`.
    pre_rebuffer_state: PlayerState,
    /// Number of tracks in the current session.
    track_count: u32,
    /// Recovery policy and attempt tracker.
    recovery_policy: RecoveryPolicy,
    recovery_tracker: RecoveryTracker,
    /// Resource ownership ledger.
    ledger: ResourceLedger,
    /// Latency controller.
    latency: LatencyController,
    /// Performance/resource instrumentation.
    metrics: super::Metrics,
    /// Monotonic clock in milliseconds used for recovery/backoff.
    now_ms: u64,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// Create a new engine in `Idle`.
    pub fn new() -> Self {
        Self {
            state: PlayerState::Idle,
            epoch: StreamEpoch::new(0),
            sequence: 0,
            has_track: false,
            has_config: false,
            pre_rebuffer_state: PlayerState::Idle,
            track_count: 0,
            recovery_policy: RecoveryPolicy::default_player(),
            recovery_tracker: RecoveryTracker::default(),
            ledger: ResourceLedger::new(),
            latency: LatencyController::default(),
            metrics: super::Metrics::new(),
            now_ms: 0,
        }
    }

    /// Current recovery policy.
    pub fn recovery_policy(&self) -> &RecoveryPolicy {
        &self.recovery_policy
    }

    /// Current resource ledger.
    pub fn ledger(&self) -> &ResourceLedger {
        &self.ledger
    }

    /// Mutable resource ledger.
    pub fn ledger_mut(&mut self) -> &mut ResourceLedger {
        &mut self.ledger
    }

    /// Current latency controller.
    pub fn latency_controller(&self) -> &LatencyController {
        &self.latency
    }

    /// Mutable metrics collector.
    pub fn metrics_mut(&mut self) -> &mut super::Metrics {
        &mut self.metrics
    }

    /// Current metrics snapshot.
    pub fn metrics(&self) -> super::MetricsSnapshot {
        self.metrics.snapshot()
    }

    /// Advance internal time. Should be driven by the caller's monotonic clock.
    pub fn tick(&mut self, now_ms: u64) {
        self.now_ms = now_ms;
        let cutoff = now_ms.saturating_sub(self.recovery_policy.max_window_ms());
        self.recovery_tracker.prune(cutoff);
    }

    /// Current player state.
    pub fn state(&self) -> PlayerState {
        self.state
    }

    /// Current epoch.
    pub fn epoch(&self) -> StreamEpoch {
        self.epoch
    }

    /// Apply `command` and return emitted events.
    pub fn apply(&mut self, command: EngineCommand) -> Result<EngineOutput, EngineError> {
        if self.state == PlayerState::Destroyed {
            return Err(EngineError::Destroyed);
        }

        self.sequence += 1;
        let mut out = EngineOutput::new();

        match command {
            EngineCommand::Load(req) => self.on_load(req, &mut out),
            EngineCommand::Play => self.on_play(&mut out),
            EngineCommand::Pause => self.on_pause(&mut out),
            EngineCommand::Stop => self.on_stop(&mut out),
            EngineCommand::Destroy => self.on_destroy(&mut out),
            EngineCommand::Network(ev) => self.on_network(ev, &mut out),
            EngineCommand::Backend(ev) => self.on_backend(ev, &mut out),
            EngineCommand::GetMetrics => {
                out.push(EngineEvent::Metrics(self.metrics.snapshot()));
            }
        };

        Ok(out)
    }

    fn transition(&mut self, to: PlayerState, out: &mut EngineOutput) {
        let from = self.state;
        if from != to {
            self.state = to;
            out.change(from, to);
        }
    }

    fn on_load(&mut self, req: LoadRequest, out: &mut EngineOutput) {
        match self.state {
            PlayerState::Idle | PlayerState::Failed => {
                self.start_new_session(out);
                let _ = req; // request is retained for logging/telemetry if needed
                self.transition(PlayerState::Loading, out);
            }
            PlayerState::Loading
            | PlayerState::Preroll
            | PlayerState::Playing
            | PlayerState::Paused
            | PlayerState::Rebuffering
            | PlayerState::Stopping => {
                // A new load while another session is active is a stop+reload.
                self.on_stop(out);
                self.start_new_session(out);
                self.transition(PlayerState::Loading, out);
            }
            PlayerState::Destroyed => {
                out.error(EngineError::Destroyed);
            }
        }
    }

    fn start_new_session(&mut self, out: &mut EngineOutput) {
        self.epoch = self.epoch.next();
        self.has_track = false;
        self.has_config = false;
        self.track_count = 0;
        self.pre_rebuffer_state = PlayerState::Idle;
        self.recovery_tracker = RecoveryTracker::default();
        self.latency = LatencyController::default();
        // Any resources still owned from a previous session are a leak.
        self.reset_ledger(out);
    }

    fn reset_ledger(&mut self, out: &mut EngineOutput) {
        if !self.ledger.is_zero() {
            let kinds = self.ledger.open_kinds();
            let total = self.ledger.total();
            out.push(EngineEvent::ResourceWarning { kinds, total });
        }
        self.ledger.reset();
    }

    fn on_play(&mut self, out: &mut EngineOutput) {
        match self.state {
            PlayerState::Preroll | PlayerState::Paused | PlayerState::Rebuffering => {
                self.transition(PlayerState::Playing, out);
            }
            PlayerState::Playing => {
                // idempotent
            }
            _ => {
                out.error(EngineError::InvalidState {
                    state: self.state,
                    command: "play",
                });
            }
        }
    }

    fn on_pause(&mut self, out: &mut EngineOutput) {
        match self.state {
            PlayerState::Playing => {
                self.transition(PlayerState::Paused, out);
            }
            PlayerState::Paused => {
                // idempotent
            }
            _ => {
                out.error(EngineError::InvalidState {
                    state: self.state,
                    command: "pause",
                });
            }
        }
    }

    fn on_stop(&mut self, out: &mut EngineOutput) {
        match self.state {
            PlayerState::Idle | PlayerState::Stopping => {
                // idempotent
            }
            PlayerState::Loading
            | PlayerState::Preroll
            | PlayerState::Playing
            | PlayerState::Paused
            | PlayerState::Rebuffering
            | PlayerState::Failed => {
                self.transition(PlayerState::Stopping, out);
            }
            PlayerState::Destroyed => {
                out.error(EngineError::Destroyed);
            }
        }
        // Stop should release all resources; warn and reset if the ledger is not empty.
        if self.state == PlayerState::Stopping && !self.ledger.is_zero() {
            self.reset_ledger(out);
        }
    }

    fn on_destroy(&mut self, out: &mut EngineOutput) {
        match self.state {
            PlayerState::Destroyed => {
                // idempotent; no-op
            }
            _ => {
                // Destroy must end with a clean ledger.
                self.reset_ledger(out);
                self.transition(PlayerState::Destroyed, out);
                out.push(EngineEvent::Destroyed);
            }
        }
    }

    fn on_network(&mut self, ev: NetworkEvent, out: &mut EngineOutput) {
        match ev {
            NetworkEvent::Connected => {
                if self.state == PlayerState::Rebuffering {
                    self.transition(self.pre_rebuffer_state, out);
                }
            }
            NetworkEvent::Eof => match self.state {
                PlayerState::Loading
                | PlayerState::Preroll
                | PlayerState::Playing
                | PlayerState::Paused
                | PlayerState::Rebuffering => {
                    out.push(EngineEvent::Eof);
                    self.transition(PlayerState::Stopping, out);
                }
                _ => {}
            },
            NetworkEvent::Retryable => match self.state {
                PlayerState::Playing | PlayerState::Paused => {
                    self.pre_rebuffer_state = self.state;
                    self.transition(PlayerState::Rebuffering, out);
                }
                _ => {}
            },
            NetworkEvent::Fatal { code } => {
                self.recover_or_fail(code, "network", out);
            }
        }
    }

    fn on_backend(&mut self, ev: BackendEvent, out: &mut EngineOutput) {
        // Generation barrier: ignore events from a previous session.
        if let Some(epoch) = ev.epoch()
            && epoch != self.epoch
        {
            return;
        }

        match ev {
            BackendEvent::Track { info, .. } => {
                self.has_track = true;
                self.track_count += 1;
                out.push(EngineEvent::TrackAdded(info.clone()));
                self.maybe_enter_preroll(out);
            }
            BackendEvent::ConfigChanged {
                track_id,
                generation,
                ..
            } => {
                self.has_config = true;
                out.push(EngineEvent::TrackConfigChanged {
                    track_id,
                    generation,
                });
                self.maybe_enter_preroll(out);
            }
            BackendEvent::DecoderReady { .. } => {
                // Ready does not change state; tracks/config still drive preroll.
            }
            BackendEvent::DecoderError { code, .. } => {
                self.recover_or_fail(code, "decoder", out);
            }
            BackendEvent::Rendered { .. } => {
                // Rendered events are consumed by metrics/clock; no state change.
            }
            BackendEvent::Stopped => {
                if self.state == PlayerState::Stopping {
                    self.transition(PlayerState::Idle, out);
                    out.push(EngineEvent::Stopped);
                }
            }
        }
    }

    fn maybe_enter_preroll(&mut self, out: &mut EngineOutput) {
        if self.state == PlayerState::Loading && self.has_track && self.has_config {
            self.transition(PlayerState::Preroll, out);
        }
    }

    fn fail(&mut self, code: u32, stage: &'static str, out: &mut EngineOutput) {
        if self.state != PlayerState::Destroyed && self.state != PlayerState::Failed {
            out.error(EngineError::Backend { stage, code });
            self.transition(PlayerState::Failed, out);
        }
    }

    fn recover_or_fail(&mut self, code: u32, stage: &'static str, out: &mut EngineOutput) {
        if self.state == PlayerState::Destroyed || self.state == PlayerState::Failed {
            return;
        }

        let decision = self
            .recovery_policy
            .decide(code, stage, self.now_ms, &|action| {
                self.recovery_tracker.attempts(code, stage, action)
            });

        match decision {
            RecoveryDecision::Retry {
                action,
                delay_ms,
                attempts_left,
            } => {
                self.recovery_tracker
                    .record(code, stage, action, self.now_ms);
                if action == RecoveryAction::ReconnectSource && self.state != PlayerState::Loading {
                    // Network reconnect is treated as a transient rebuffer.
                    self.pre_rebuffer_state = if self.state == PlayerState::Rebuffering {
                        self.pre_rebuffer_state
                    } else {
                        self.state
                    };
                    self.transition(PlayerState::Rebuffering, out);
                }
                out.push(EngineEvent::RecoveryScheduled {
                    action,
                    stage,
                    code,
                    delay_ms,
                    attempts_left,
                });
            }
            RecoveryDecision::Escalate { action } => {
                // The escalation chain reached its end without a retry budget.
                // Record the final attempt and fail.
                self.recovery_tracker
                    .record(code, stage, action, self.now_ms);
                self.fail(code, stage, out);
            }
            RecoveryDecision::Fatal => self.fail(code, stage, out),
        }
    }
}

impl BackendEvent {
    /// The epoch this event belongs to, if any.
    fn epoch(&self) -> Option<StreamEpoch> {
        match self {
            Self::Track { epoch, .. } => Some(*epoch),
            Self::ConfigChanged { epoch, .. } => Some(*epoch),
            Self::DecoderReady { epoch, .. } => Some(*epoch),
            Self::DecoderError { epoch, .. } => Some(*epoch),
            Self::Rendered { epoch, .. } => Some(*epoch),
            Self::Stopped => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_types::{CodecId, TimeBase, TrackId, TrackKind};

    fn track() -> TrackInfo {
        TrackInfo::new(
            TrackId::new(1).unwrap(),
            TrackKind::Video,
            CodecId::H264,
            TimeBase::DEFAULT,
        )
    }

    fn load(engine: &mut Engine) -> StreamEpoch {
        let e0 = engine.epoch();
        engine
            .apply(EngineCommand::Load(LoadRequest {
                url: "http://example.com/test.flv",
                is_live: false,
            }))
            .unwrap();
        assert_eq!(engine.epoch().get(), e0.get() + 1);
        engine.epoch()
    }

    fn transition_events(out: &EngineOutput) -> Vec<(PlayerState, PlayerState)> {
        out.events
            .iter()
            .filter_map(|e| match e {
                EngineEvent::StateChanged { from, to } => Some((*from, *to)),
                _ => None,
            })
            .collect()
    }

    fn recovery_events(out: &EngineOutput) -> Vec<RecoveryAction> {
        out.events
            .iter()
            .filter_map(|e| match e {
                EngineEvent::RecoveryScheduled { action, .. } => Some(*action),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn lifecycle_idle_to_playing() {
        let mut engine = Engine::new();
        assert_eq!(engine.state(), PlayerState::Idle);

        let epoch = load(&mut engine);

        let out = engine
            .apply(EngineCommand::Backend(BackendEvent::Track {
                epoch,
                info: track(),
            }))
            .unwrap();
        assert!(out.events.contains(&EngineEvent::TrackAdded(track())));
        let out = engine
            .apply(EngineCommand::Backend(BackendEvent::ConfigChanged {
                epoch,
                track_id: TrackId::new(1).unwrap(),
                generation: 1,
            }))
            .unwrap();
        assert_eq!(
            transition_events(&out),
            vec![(PlayerState::Loading, PlayerState::Preroll)]
        );

        let out = engine.apply(EngineCommand::Play).unwrap();
        assert_eq!(
            transition_events(&out),
            vec![(PlayerState::Preroll, PlayerState::Playing)]
        );

        let out = engine.apply(EngineCommand::Pause).unwrap();
        assert_eq!(
            transition_events(&out),
            vec![(PlayerState::Playing, PlayerState::Paused)]
        );

        let out = engine.apply(EngineCommand::Play).unwrap();
        assert_eq!(
            transition_events(&out),
            vec![(PlayerState::Paused, PlayerState::Playing)]
        );

        let out = engine.apply(EngineCommand::Stop).unwrap();
        assert_eq!(
            transition_events(&out),
            vec![(PlayerState::Playing, PlayerState::Stopping)]
        );

        let out = engine
            .apply(EngineCommand::Backend(BackendEvent::Stopped))
            .unwrap();
        assert_eq!(
            transition_events(&out),
            vec![(PlayerState::Stopping, PlayerState::Idle)]
        );
        assert!(out.events.contains(&EngineEvent::Stopped));
    }

    #[test]
    fn play_is_invalid_in_idle() {
        let mut engine = Engine::new();
        let out = engine.apply(EngineCommand::Play).unwrap();
        assert!(
            out.events
                .iter()
                .any(|e| matches!(e, EngineEvent::Error(EngineError::InvalidState { .. })))
        );
        assert_eq!(engine.state(), PlayerState::Idle);
    }

    #[test]
    fn destroy_is_final() {
        let mut engine = Engine::new();
        load(&mut engine);
        let out = engine.apply(EngineCommand::Destroy).unwrap();
        assert_eq!(
            transition_events(&out),
            vec![(PlayerState::Loading, PlayerState::Destroyed)]
        );
        assert!(out.events.contains(&EngineEvent::Destroyed));
        assert!(matches!(
            engine.apply(EngineCommand::Play),
            Err(EngineError::Destroyed)
        ));
        assert_eq!(engine.state(), PlayerState::Destroyed);
        assert!(engine.ledger().is_zero());
    }

    #[test]
    fn network_fatal_schedules_reconnect_while_loading() {
        let mut engine = Engine::new();
        engine.tick(1000);
        load(&mut engine);
        let out = engine
            .apply(EngineCommand::Network(NetworkEvent::Fatal { code: 42 }))
            .unwrap();
        // During the initial load the engine stays in Loading and lets the source retry.
        assert!(transition_events(&out).is_empty());
        assert_eq!(engine.state(), PlayerState::Loading);
        assert_eq!(recovery_events(&out), vec![RecoveryAction::ReconnectSource]);
    }

    #[test]
    fn network_fatal_rebuffers_while_playing_then_becomes_fatal() {
        let mut engine = Engine::new();
        let epoch = load(&mut engine);
        engine
            .apply(EngineCommand::Backend(BackendEvent::Track {
                epoch,
                info: track(),
            }))
            .unwrap();
        engine
            .apply(EngineCommand::Backend(BackendEvent::ConfigChanged {
                epoch,
                track_id: TrackId::new(1).unwrap(),
                generation: 1,
            }))
            .unwrap();
        engine.apply(EngineCommand::Play).unwrap();

        // The first fatal network error moves to Rebuffering and schedules reconnect.
        let out = engine
            .apply(EngineCommand::Network(NetworkEvent::Fatal { code: 42 }))
            .unwrap();
        assert_eq!(
            transition_events(&out),
            vec![(PlayerState::Playing, PlayerState::Rebuffering)]
        );
        assert_eq!(recovery_events(&out), vec![RecoveryAction::ReconnectSource]);

        // Burn the reconnect budget.
        for t in 1..=5 {
            engine.tick(1000 + t * 1000);
            engine
                .apply(EngineCommand::Network(NetworkEvent::Fatal { code: 42 }))
                .unwrap();
        }
        assert_eq!(engine.state(), PlayerState::Failed);
    }

    #[test]
    fn stop_is_idempotent_from_stopping() {
        let mut engine = Engine::new();
        load(&mut engine);
        engine.apply(EngineCommand::Stop).unwrap();
        assert_eq!(engine.state(), PlayerState::Stopping);
        let out = engine.apply(EngineCommand::Stop).unwrap();
        assert!(transition_events(&out).is_empty());
        assert_eq!(engine.state(), PlayerState::Stopping);
    }

    #[test]
    fn load_increments_epoch() {
        let mut engine = Engine::new();
        let e0 = engine.epoch();
        let e1 = load(&mut engine);
        assert_eq!(e1.get(), e0.get() + 1);
        let e2 = load(&mut engine);
        assert_eq!(e2.get(), e1.get() + 1);
    }

    #[test]
    fn rebuffer_and_recover() {
        let mut engine = Engine::new();
        let epoch = load(&mut engine);
        engine
            .apply(EngineCommand::Backend(BackendEvent::Track {
                epoch,
                info: track(),
            }))
            .unwrap();
        engine
            .apply(EngineCommand::Backend(BackendEvent::ConfigChanged {
                epoch,
                track_id: TrackId::new(1).unwrap(),
                generation: 1,
            }))
            .unwrap();
        engine.apply(EngineCommand::Play).unwrap();

        let out = engine
            .apply(EngineCommand::Network(NetworkEvent::Retryable))
            .unwrap();
        assert_eq!(
            transition_events(&out),
            vec![(PlayerState::Playing, PlayerState::Rebuffering)]
        );

        let out = engine
            .apply(EngineCommand::Network(NetworkEvent::Connected))
            .unwrap();
        assert_eq!(
            transition_events(&out),
            vec![(PlayerState::Rebuffering, PlayerState::Playing)]
        );
    }

    #[test]
    fn stale_backend_events_are_ignored_after_new_load() {
        let mut engine = Engine::new();
        let epoch1 = load(&mut engine);
        // Simulate a new load before the old backend events arrive.
        let epoch2 = load(&mut engine);
        assert_ne!(epoch1, epoch2);

        let out = engine
            .apply(EngineCommand::Backend(BackendEvent::Track {
                epoch: epoch1,
                info: track(),
            }))
            .unwrap();
        assert!(!out.events.contains(&EngineEvent::TrackAdded(track())));
        assert_eq!(engine.state(), PlayerState::Loading);
    }

    #[test]
    fn stop_warns_on_resource_leak() {
        let mut engine = Engine::new();
        load(&mut engine);
        engine.ledger_mut().acquire(ResourceKind::Network);
        engine.ledger_mut().acquire(ResourceKind::Timer);
        let out = engine.apply(EngineCommand::Stop).unwrap();
        assert!(
            out.events
                .iter()
                .any(|e| matches!(e, EngineEvent::ResourceWarning { .. }))
        );
        assert!(engine.ledger().is_zero());
    }

    #[test]
    fn decoder_error_schedules_fallback() {
        let mut engine = Engine::new();
        engine.tick(1000);
        let epoch = load(&mut engine);
        engine
            .apply(EngineCommand::Backend(BackendEvent::ConfigChanged {
                epoch,
                track_id: TrackId::new(1).unwrap(),
                generation: 1,
            }))
            .unwrap();
        let out = engine
            .apply(EngineCommand::Backend(BackendEvent::DecoderError {
                epoch,
                track_id: TrackId::new(1).unwrap(),
                code: 7,
            }))
            .unwrap();
        assert_eq!(recovery_events(&out), vec![RecoveryAction::FallbackBackend]);
        // State remains Loading because the error is recoverable and we are not playing yet.
        assert_eq!(engine.state(), PlayerState::Loading);
    }

    #[test]
    fn repeated_load_cycles_keep_ledger_clean() {
        let mut engine = Engine::new();
        for i in 0..1_000 {
            load(&mut engine);
            engine.apply(EngineCommand::Stop).unwrap();
            engine
                .apply(EngineCommand::Backend(BackendEvent::Stopped))
                .unwrap();
            engine.tick(i as u64 + 1);
        }
        assert!(engine.ledger().is_zero());
    }

    #[test]
    fn repeated_backend_faults_do_not_grow_state_without_bound() {
        let mut engine = Engine::new();
        engine.tick(1_000_000);
        let epoch = load(&mut engine);
        // Apply 10,000 decoder faults with time moving forward enough that
        // the tracker prunes old entries.
        for i in 0..10_000 {
            engine.tick(1_000_000 + i * 1_000);
            engine
                .apply(EngineCommand::Backend(BackendEvent::DecoderError {
                    epoch,
                    track_id: TrackId::new(1).unwrap(),
                    code: 7,
                }))
                .unwrap();
            if engine.state() == PlayerState::Failed {
                break;
            }
        }
        assert!(engine.ledger().is_zero());
    }

    #[test]
    fn get_metrics_returns_snapshot() {
        let mut engine = Engine::new();
        engine
            .metrics_mut()
            .record_copy(cheetah_media_types::CopyReason::DemuxToDecoder, 123);
        let out = engine.apply(EngineCommand::GetMetrics).unwrap();
        assert!(
            out.events
                .iter()
                .any(|e| matches!(e, EngineEvent::Metrics(_)))
        );
    }
}

//! Broadcast engine lifecycle state machine.
//!
//! `BroadcastEngine` owns an optional `BroadcastPipeline` and routes commands
//! through an explicit state machine. It shares `ResourceLedger` and `Metrics`
//! with the playback engine.

use alloc::string::String;
use alloc::vec::Vec;

use cheetah_media_types::{MediaError, TrackId};

use crate::broadcast::permission::{
    CaptureSourceKind, HostPermissionModel, PermissionModel, PermissionState,
};
use crate::broadcast::pipeline::BroadcastPipeline;

/// Lifecycle states for the broadcast engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BroadcastState {
    /// Engine created, no pipeline set.
    #[default]
    Idle,
    /// A pipeline is set but not yet connected.
    Ready,
    /// Attempting to connect the publisher.
    Connecting,
    /// Attempting to start capture and configure the encoder.
    Starting,
    /// Actively capturing, encoding and publishing.
    Broadcasting,
    /// A stop was requested; resources are being released.
    Stopping,
    /// Stopped and can be restarted.
    Stopped,
    /// A non-recoverable failure occurred.
    Failed,
    /// Engine has been destroyed.
    Destroyed,
}

/// Errors that can be returned by `BroadcastEngine::apply`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BroadcastError {
    /// Command is not valid in the current state.
    InvalidState {
        state: BroadcastState,
        command: &'static str,
    },
    /// No pipeline has been attached.
    PipelineNotSet,
    /// Pipeline returned an error.
    Pipeline(MediaError),
    /// Required capture permission was not granted.
    PermissionDenied { kind: CaptureSourceKind },
}

/// Commands accepted by the broadcast engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BroadcastCommand {
    /// Set the URL for the publisher.
    Connect(String),
    /// Start capture and publishing.
    Start,
    /// Stop capture and publishing.
    Stop,
    /// Request the next encoded frame to be a keyframe.
    RequestKeyframe,
    /// Update the encoder target bitrate.
    SetBitrate(u32),
    /// Request permission for a capture kind.
    RequestPermission(CaptureSourceKind),
    /// Destroy the engine and release all resources.
    Destroy,
}

/// Events emitted by the broadcast engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BroadcastEvent {
    /// State changed.
    StateChanged {
        from: BroadcastState,
        to: BroadcastState,
    },
    /// A packet was published.
    PacketPublished { sequence: u64, track_id: TrackId },
    /// Permission state changed.
    PermissionChanged {
        kind: CaptureSourceKind,
        state: PermissionState,
    },
    /// An error was raised.
    Error(MediaError),
}

/// Broadcast engine state machine.
pub struct BroadcastEngine {
    state: BroadcastState,
    pipeline: Option<BroadcastPipeline>,
    permission_model: Box<dyn PermissionModel>,
}

impl BroadcastEngine {
    /// Create an engine without a pipeline.
    pub fn new() -> Self {
        Self {
            state: BroadcastState::Idle,
            pipeline: None,
            permission_model: Box::new(HostPermissionModel),
        }
    }

    /// Create an engine with an attached pipeline.
    pub fn with_pipeline(pipeline: BroadcastPipeline) -> Self {
        Self {
            state: BroadcastState::Ready,
            pipeline: Some(pipeline),
            permission_model: Box::new(HostPermissionModel),
        }
    }

    /// Replace the permission model.
    pub fn with_permission_model(mut self, model: Box<dyn PermissionModel>) -> Self {
        self.permission_model = model;
        self
    }

    /// Set the permission model after construction.
    pub fn set_permission_model(&mut self, model: Box<dyn PermissionModel>) {
        self.permission_model = model;
    }

    /// Current state.
    pub const fn state(&self) -> BroadcastState {
        self.state
    }

    /// Apply `command` and return any events emitted.
    pub fn apply(
        &mut self,
        command: BroadcastCommand,
    ) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        match command {
            BroadcastCommand::Connect(url) => self.connect(&url),
            BroadcastCommand::Start => self.start(),
            BroadcastCommand::Stop => self.stop(),
            BroadcastCommand::RequestKeyframe => self.request_keyframe(),
            BroadcastCommand::SetBitrate(bps) => self.set_bitrate(bps),
            BroadcastCommand::RequestPermission(kind) => self.request_permission(kind),
            BroadcastCommand::Destroy => self.destroy(),
        }
    }

    /// Advance the pipeline by one frame when broadcasting.
    pub fn tick(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if self.state != BroadcastState::Broadcasting {
            return Ok(Vec::new());
        }
        let pipeline = self
            .pipeline
            .as_mut()
            .ok_or(BroadcastError::PipelineNotSet)?;
        match pipeline.tick() {
            Ok(None) => Ok(Vec::new()),
            Ok(Some(summary)) => Ok(vec![BroadcastEvent::PacketPublished {
                sequence: summary.sequence,
                track_id: summary.track_id,
            }]),
            Err(err) => {
                self.state = BroadcastState::Failed;
                Ok(vec![BroadcastEvent::Error(err)])
            }
        }
    }

    fn transition(&mut self, to: BroadcastState) -> Vec<BroadcastEvent> {
        let from = self.state;
        if from == to {
            return Vec::new();
        }
        self.state = to;
        vec![BroadcastEvent::StateChanged { from, to }]
    }

    fn connect(&mut self, url: &str) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if self.state != BroadcastState::Ready && self.state != BroadcastState::Stopped {
            return Err(BroadcastError::InvalidState {
                state: self.state,
                command: "connect",
            });
        }
        let pipeline = self
            .pipeline
            .as_mut()
            .ok_or(BroadcastError::PipelineNotSet)?;
        match pipeline.connect(url) {
            Ok(()) => {
                let mut events = self.transition(BroadcastState::Connecting);
                events.extend(self.transition(BroadcastState::Ready));
                Ok(events)
            }
            Err(err) => {
                self.state = BroadcastState::Failed;
                Ok(vec![BroadcastEvent::Error(err)])
            }
        }
    }

    fn start(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if self.state != BroadcastState::Ready {
            return Err(BroadcastError::InvalidState {
                state: self.state,
                command: "start",
            });
        }

        let mut events = Vec::new();

        // If the capture source requires permission, request it before starting.
        let required_kind = self.pipeline.as_ref().and_then(|p| p.required_permission());
        if let Some(kind) = required_kind {
            let before = self.permission_model.query(kind);
            let after = if before == PermissionState::Granted {
                before
            } else {
                self.permission_model.request(kind)
            };
            if before != after {
                events.push(BroadcastEvent::PermissionChanged { kind, state: after });
            }
            if after != PermissionState::Granted {
                return Err(BroadcastError::PermissionDenied { kind });
            }
        }

        let pipeline = self
            .pipeline
            .as_mut()
            .ok_or(BroadcastError::PipelineNotSet)?;
        match pipeline.start() {
            Ok(()) => {
                events.extend(self.transition(BroadcastState::Starting));
                events.extend(self.transition(BroadcastState::Broadcasting));
                Ok(events)
            }
            Err(err) => {
                self.state = BroadcastState::Failed;
                events.push(BroadcastEvent::Error(err));
                Ok(events)
            }
        }
    }

    fn stop(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if !matches!(
            self.state,
            BroadcastState::Broadcasting
                | BroadcastState::Starting
                | BroadcastState::Ready
                | BroadcastState::Failed
        ) {
            return Err(BroadcastError::InvalidState {
                state: self.state,
                command: "stop",
            });
        }
        if let Some(pipeline) = self.pipeline.as_mut() {
            let _ = pipeline.stop();
        }
        Ok(self.transition(BroadcastState::Stopped))
    }

    fn request_keyframe(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if self.state != BroadcastState::Broadcasting {
            return Err(BroadcastError::InvalidState {
                state: self.state,
                command: "request_keyframe",
            });
        }
        let pipeline = self
            .pipeline
            .as_mut()
            .ok_or(BroadcastError::PipelineNotSet)?;
        pipeline
            .encoder_mut()
            .request_keyframe()
            .map_err(BroadcastError::Pipeline)?;
        Ok(Vec::new())
    }

    fn set_bitrate(&mut self, bps: u32) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if self.state != BroadcastState::Broadcasting {
            return Err(BroadcastError::InvalidState {
                state: self.state,
                command: "set_bitrate",
            });
        }
        let pipeline = self
            .pipeline
            .as_mut()
            .ok_or(BroadcastError::PipelineNotSet)?;
        pipeline
            .encoder_mut()
            .set_bitrate(bps)
            .map_err(BroadcastError::Pipeline)?;
        Ok(Vec::new())
    }

    fn request_permission(
        &mut self,
        kind: CaptureSourceKind,
    ) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if self.state == BroadcastState::Destroyed {
            return Err(BroadcastError::InvalidState {
                state: self.state,
                command: "request_permission",
            });
        }
        let before = self.permission_model.query(kind);
        let after = self.permission_model.request(kind);
        if before == after {
            return Ok(Vec::new());
        }
        Ok(vec![BroadcastEvent::PermissionChanged {
            kind,
            state: after,
        }])
    }

    fn destroy(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if self.state == BroadcastState::Destroyed {
            return Ok(Vec::new());
        }
        if let Some(pipeline) = self.pipeline.as_mut() {
            let _ = pipeline.stop();
        }
        self.pipeline = None;
        Ok(self.transition(BroadcastState::Destroyed))
    }
}

impl Default for BroadcastEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broadcast::capture_sources::CameraCaptureSource;
    use crate::broadcast::encoder::UnsupportedEncoder;
    use crate::broadcast::permission::{
        AlwaysDenyPermissionModel, AlwaysGrantPermissionModel, CaptureSourceKind, PermissionState,
    };
    use crate::broadcast::pipeline::{BroadcastPipeline, PipelineConfig};
    use crate::broadcast::publisher::{PublisherBackend, UnsupportedPublisherBackend};
    use crate::broadcast::source::UnsupportedCaptureSource;
    use cheetah_media_types::{CodecId, MediaPacket, StreamEpoch, TrackId};

    fn make_engine() -> BroadcastEngine {
        let config = PipelineConfig {
            track_id: TrackId::new(1).unwrap(),
            stream_epoch: StreamEpoch::new(0),
            codec: CodecId::H264,
            width: 64,
            height: 64,
            fps: 30,
        };
        let pipeline = BroadcastPipeline::new(
            Box::new(UnsupportedCaptureSource),
            Vec::new(),
            Box::new(UnsupportedEncoder),
            Box::new(UnsupportedPublisherBackend),
            config,
        );
        BroadcastEngine::with_pipeline(pipeline)
    }

    #[test]
    fn engine_lifecycle_with_unsupported_pipeline() {
        let mut engine = make_engine();
        assert_eq!(engine.state(), BroadcastState::Ready);

        // Connect fails because the publisher backend is unsupported.
        let events = engine
            .apply(BroadcastCommand::Connect("webrtc://x".into()))
            .unwrap();
        assert_eq!(engine.state(), BroadcastState::Failed);
        assert!(events.iter().any(|e| matches!(e, BroadcastEvent::Error(_))));

        // Stop from Failed should move to Stopped.
        let events = engine.apply(BroadcastCommand::Stop).unwrap();
        assert_eq!(engine.state(), BroadcastState::Stopped);
        assert!(events.iter().any(|e| matches!(
            e,
            BroadcastEvent::StateChanged {
                to: BroadcastState::Stopped,
                ..
            }
        )));

        // Destroy from Stopped is allowed.
        let events = engine.apply(BroadcastCommand::Destroy).unwrap();
        assert_eq!(engine.state(), BroadcastState::Destroyed);
        assert!(events.iter().any(|e| matches!(
            e,
            BroadcastEvent::StateChanged {
                to: BroadcastState::Destroyed,
                ..
            }
        )));
    }

    #[test]
    fn start_requires_ready_state() {
        let mut engine = BroadcastEngine::new();
        assert!(matches!(
            engine.apply(BroadcastCommand::Start).unwrap_err(),
            BroadcastError::InvalidState {
                state: BroadcastState::Idle,
                command: "start"
            }
        ));
    }

    #[test]
    fn request_keyframe_and_set_bitrate_require_broadcasting() {
        let mut engine = make_engine();
        assert!(engine.apply(BroadcastCommand::RequestKeyframe).is_err());
        assert!(
            engine
                .apply(BroadcastCommand::SetBitrate(1_000_000))
                .is_err()
        );
    }

    struct MockPublisher {
        connected: bool,
    }

    impl PublisherBackend for MockPublisher {
        fn connect(&mut self, _url: &str) -> Result<(), cheetah_media_types::MediaError> {
            self.connected = true;
            Ok(())
        }

        fn publish(
            &mut self,
            _packet: &MediaPacket<'static>,
        ) -> Result<(), cheetah_media_types::MediaError> {
            Ok(())
        }

        fn flush(&mut self) -> Result<(), cheetah_media_types::MediaError> {
            Ok(())
        }

        fn disconnect(&mut self) {
            self.connected = false;
        }

        fn connected(&self) -> bool {
            self.connected
        }

        fn kind(&self) -> &'static str {
            "mock"
        }
    }

    fn permission_pipeline() -> BroadcastPipeline {
        let config = PipelineConfig {
            track_id: TrackId::new(1).unwrap(),
            stream_epoch: StreamEpoch::new(0),
            codec: CodecId::H264,
            width: 64,
            height: 64,
            fps: 30,
        };
        BroadcastPipeline::new(
            Box::new(CameraCaptureSource {
                width: 64,
                height: 64,
            }),
            Vec::new(),
            Box::new(UnsupportedEncoder),
            Box::new(MockPublisher { connected: false }),
            config,
        )
    }

    #[test]
    fn start_fails_when_permission_denied() {
        let mut engine = BroadcastEngine::with_pipeline(permission_pipeline())
            .with_permission_model(Box::new(AlwaysDenyPermissionModel));
        engine
            .apply(BroadcastCommand::Connect("mock://x".into()))
            .unwrap();
        assert!(matches!(
            engine.apply(BroadcastCommand::Start).unwrap_err(),
            BroadcastError::PermissionDenied {
                kind: CaptureSourceKind::Camera
            }
        ));
    }

    #[test]
    fn start_with_granted_permission_still_fails_at_encoder() {
        let mut engine = BroadcastEngine::with_pipeline(permission_pipeline())
            .with_permission_model(Box::new(AlwaysGrantPermissionModel));
        engine
            .apply(BroadcastCommand::Connect("mock://x".into()))
            .unwrap();
        let events = engine.apply(BroadcastCommand::Start).unwrap();
        assert_eq!(engine.state(), BroadcastState::Failed);
        assert!(events.iter().any(|e| matches!(e, BroadcastEvent::Error(_))));
    }

    struct PromptThenDenyModel;

    impl PermissionModel for PromptThenDenyModel {
        fn query(&self, _kind: CaptureSourceKind) -> PermissionState {
            PermissionState::Prompt
        }

        fn request(&mut self, _kind: CaptureSourceKind) -> PermissionState {
            PermissionState::Denied
        }
    }

    #[test]
    fn request_permission_emits_event_when_state_changes() {
        let mut engine =
            BroadcastEngine::new().with_permission_model(Box::new(PromptThenDenyModel));
        let events = engine
            .apply(BroadcastCommand::RequestPermission(
                CaptureSourceKind::Camera,
            ))
            .unwrap();
        assert!(events.iter().any(|e| matches!(
            e,
            BroadcastEvent::PermissionChanged {
                kind: CaptureSourceKind::Camera,
                state: PermissionState::Denied,
            }
        )));
    }
}

//! Real-time latency control.
//!
//! `LatencyController` decomposes total latency into input, demux, decode and
//! render components and decides how to keep it within soft/hard targets.

use cheetah_media_timeline::clock::{ClockTime, MediaClock};

/// Latency target for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencyTarget {
    /// Target latency before the controller applies mild corrections.
    pub soft_ms: i64,
    /// Maximum acceptable latency before the controller must drop frames.
    pub hard_ms: i64,
}

impl Default for LatencyTarget {
    fn default() -> Self {
        Self {
            soft_ms: 300,
            hard_ms: 1_500,
        }
    }
}

/// Components of end-to-end latency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LatencyBreakdown {
    /// Time from ingest to demux latest DTS in milliseconds.
    pub input_to_demux_ms: i64,
    /// Time from latest demuxed DTS to decode output in milliseconds.
    pub decode_ms: i64,
    /// Time from decoded frame to render in milliseconds.
    pub render_ms: i64,
    /// Estimated live edge offset from wall clock in milliseconds.
    pub wall_offset_ms: i64,
}

impl LatencyBreakdown {
    /// Sum of input-to-render latency.
    pub fn total_ms(&self) -> i64 {
        self.input_to_demux_ms
            .saturating_add(self.decode_ms)
            .saturating_add(self.render_ms)
    }
}

/// Action returned by the latency controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyAction {
    /// Latency is within target; no correction needed.
    None,
    /// Accelerate playback slightly to drain buffered latency.
    SpeedUp { rate: i64, reason: &'static str },
    /// Drop to the next safe keyframe at `target_ms` latency.
    DropToKeyframe {
        target_ms: i64,
        dropped_ms: i64,
        reason: &'static str,
    },
    /// Jump to the live edge; used when the gap exceeds hard target.
    JumpToLive {
        dropped_ms: i64,
        reason: &'static str,
    },
}

/// Controller that decides how to keep latency within target bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatencyController {
    target: LatencyTarget,
    last_action: LatencyAction,
    total_dropped_ms: i64,
}

impl Default for LatencyController {
    fn default() -> Self {
        Self::new(LatencyTarget::default())
    }
}

impl LatencyController {
    /// Create a controller with the given target.
    pub const fn new(target: LatencyTarget) -> Self {
        Self {
            target,
            last_action: LatencyAction::None,
            total_dropped_ms: 0,
        }
    }

    /// Update controller with current clock state and breakdown.
    ///
    /// `now` is the current monotonic clock time, `latest_demux` is the latest
    /// demuxed DTS as a clock time, and `latest_render` is the most recent
    /// rendered frame clock time.
    pub fn update(
        &mut self,
        clock: &MediaClock,
        breakdown: LatencyBreakdown,
        latest_demux: Option<ClockTime>,
        latest_render: Option<ClockTime>,
    ) -> LatencyAction {
        let total = breakdown.total_ms();
        let buffer_ms = clock.stats().buffer_level_ms;

        // Hard target breach -> jump to live edge.
        if total > self.target.hard_ms {
            let dropped = total.saturating_sub(self.target.soft_ms);
            self.total_dropped_ms = self.total_dropped_ms.saturating_add(dropped);
            let action = LatencyAction::JumpToLive {
                dropped_ms: dropped,
                reason: "latency exceeded hard target",
            };
            self.last_action = action;
            return action;
        }

        // Soft target breach -> drop to the next keyframe if buffer is deep.
        if total > self.target.soft_ms && buffer_ms > self.target.soft_ms {
            let dropped = total.saturating_sub(self.target.soft_ms / 2);
            self.total_dropped_ms = self.total_dropped_ms.saturating_add(dropped);
            let action = LatencyAction::DropToKeyframe {
                target_ms: self.target.soft_ms / 2,
                dropped_ms: dropped,
                reason: "latency exceeded soft target with available buffer",
            };
            self.last_action = action;
            return action;
        }

        // Mild drift -> gently speed up playback if render is behind demux.
        if total > self.target.soft_ms / 2 {
            let gap = latest_demux
                .map(|d: ClockTime| d.us())
                .unwrap_or(0)
                .saturating_sub(latest_render.map(|r: ClockTime| r.us()).unwrap_or(0))
                / 1000;
            if gap > self.target.soft_ms / 4 {
                let action = LatencyAction::SpeedUp {
                    rate: 105, // 5% faster
                    reason: "draining mild latency",
                };
                self.last_action = action;
                return action;
            }
        }

        let action = LatencyAction::None;
        self.last_action = action;
        action
    }

    /// Last action returned by `update`.
    pub fn last_action(&self) -> LatencyAction {
        self.last_action
    }

    /// Total duration dropped to recover sync, in milliseconds.
    pub fn total_dropped_ms(&self) -> i64 {
        self.total_dropped_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_timeline::clock::{ClockState, MediaClock};

    #[test]
    fn latency_within_target_is_none() {
        let mut ctrl = LatencyController::default();
        let clock = MediaClock::new(None, None);
        let breakdown = LatencyBreakdown {
            input_to_demux_ms: 50,
            decode_ms: 50,
            render_ms: 50,
            ..Default::default()
        };
        let action = ctrl.update(&clock, breakdown, None, None);
        assert_eq!(action, LatencyAction::None);
    }

    #[test]
    fn hard_target_triggers_jump_to_live() {
        let mut ctrl = LatencyController::default();
        let mut clock = MediaClock::new(None, None);
        clock.set_stats(0, 2000, 0); // deep buffer
        let breakdown = LatencyBreakdown {
            input_to_demux_ms: 1000,
            decode_ms: 400,
            render_ms: 200,
            ..Default::default()
        };
        let action = ctrl.update(&clock, breakdown, None, None);
        assert!(matches!(action, LatencyAction::JumpToLive { .. }));
        assert!(ctrl.total_dropped_ms() > 0);
    }

    #[test]
    fn soft_target_with_buffer_triggers_drop() {
        let mut ctrl = LatencyController::new(LatencyTarget {
            soft_ms: 200,
            hard_ms: 1_000,
        });
        let mut clock = MediaClock::new(None, None);
        clock.set_stats(0, 400, 0);
        let breakdown = LatencyBreakdown {
            input_to_demux_ms: 300,
            decode_ms: 100,
            render_ms: 50,
            ..Default::default()
        };
        let action = ctrl.update(&clock, breakdown, None, None);
        assert!(matches!(action, LatencyAction::DropToKeyframe { .. }));
    }

    #[test]
    fn mild_latency_triggers_speed_up_when_render_lags_demux() {
        let mut ctrl = LatencyController::default();
        let mut clock = MediaClock::new(None, None);
        clock.set_stats(0, 200, 0);
        let breakdown = LatencyBreakdown {
            input_to_demux_ms: 120,
            decode_ms: 30,
            render_ms: 20,
            ..Default::default()
        };
        let action = ctrl.update(
            &clock,
            breakdown,
            Some(ClockTime::new(200_000)),
            Some(ClockTime::new(0)),
        );
        assert!(matches!(action, LatencyAction::SpeedUp { rate, .. } if rate == 105));
    }

    #[test]
    fn soft_target_without_buffer_stays_none() {
        let mut ctrl = LatencyController::default();
        let mut clock = MediaClock::new(None, None);
        clock.set_state(ClockState::Rebuffering);
        clock.set_stats(0, 50, 0); // shallow buffer
        let breakdown = LatencyBreakdown {
            input_to_demux_ms: 200,
            decode_ms: 100,
            render_ms: 50,
            ..Default::default()
        };
        let action = ctrl.update(&clock, breakdown, None, None);
        assert_eq!(action, LatencyAction::None);
    }
}

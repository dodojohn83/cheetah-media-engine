//! A/V sync and catch-up policy.
//!
//! `AvSync` decides when to render, hold, or drop a video frame relative to
//! the audio render clock. Audio is the master clock when available; otherwise a
//! wall-clock baseline is used. Large discontinuities create a new sync baseline.

use crate::clock::{ClockState, ClockTime, MediaClock, TimelineError};
use cheetah_media_types::{MediaTime, StreamEpoch};

/// Decision returned by the A/V synchronizer for a video sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDecision {
    /// Render the sample at the given monotonic clock time.
    Render { target: ClockTime },
    /// Drop the sample but keep the reference chain intact.
    Drop { reason: &'static str },
    /// Hold the sample until at least the given clock time.
    Hold { until: ClockTime },
}

/// Audio underflow/overflow policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioUnderflowPolicy {
    /// Insert silence for the missing samples.
    InsertSilence,
    /// Wait for the next audio packet.
    Wait,
}

/// A/V sync controller.
#[derive(Debug, Default)]
pub struct AvSync {
    clock: MediaClock,
    audio_available: bool,
    last_audio_ms: Option<i64>,
    last_video_ms: Option<i64>,
    /// Maximum allowed A/V drift in milliseconds before corrective action.
    max_drift_ms: i64,
    /// Video late threshold in milliseconds.
    late_threshold_ms: i64,
    /// Whether a frame under the late threshold is still rendered if not a keyframe.
    drop_non_reference: bool,
    /// Running maximum observed |drift| in milliseconds.
    max_observed_drift_ms: i64,
}

impl AvSync {
    /// Create a new A/V sync controller.
    pub fn new(max_drift_ms: i64, late_threshold_ms: i64) -> Self {
        Self {
            clock: MediaClock::new(None, None),
            audio_available: false,
            last_audio_ms: None,
            last_video_ms: None,
            max_drift_ms,
            late_threshold_ms,
            drop_non_reference: false,
            max_observed_drift_ms: 0,
        }
    }

    /// Feed an audio sample and update the audio master clock.
    pub fn feed_audio(&mut self, time: MediaTime, _duration_ms: i64, epoch: StreamEpoch) {
        if let Ok(render_time) = self.clock.update(time, epoch) {
            self.audio_available = true;
            // Use raw presentation time for drift measurements; the monotonic
            // render clock is only for scheduling.
            self.last_audio_ms = time
                .pts_ms()
                .or_else(|| time.dts_ms())
                .or(Some(render_time.ms()));
        }
    }

    /// Feed a video frame and return the sync decision.
    ///
    /// `is_keyframe` is true for independent frames (IDR/CRA/all-intra). When the
    /// video is behind the audio clock by more than `late_threshold_ms`, only
    /// keyframes are dropped to preserve the reference chain unless
    /// `drop_non_reference` has been enabled by a higher-level policy.
    pub fn feed_video(
        &mut self,
        time: MediaTime,
        is_keyframe: bool,
        epoch: StreamEpoch,
    ) -> Result<SyncDecision, TimelineError> {
        let render_time = self.clock.update(time, epoch)?;
        let video_ms = time
            .pts_ms()
            .or_else(|| time.dts_ms())
            .unwrap_or(render_time.ms());
        self.last_video_ms = Some(video_ms);

        let audio_ms = self.last_audio_ms.unwrap_or(video_ms);
        let drift_ms = video_ms.saturating_sub(audio_ms);
        if drift_ms.abs() > self.max_observed_drift_ms {
            self.max_observed_drift_ms = drift_ms.abs();
        }

        // Large forward jump: create a discontinuity and re-base audio clock.
        if self.audio_available && drift_ms > self.max_drift_ms {
            self.clock.add_dropped(drift_ms);
            self.clock.set_state(ClockState::CatchUp);
            if is_keyframe {
                return Ok(SyncDecision::Drop {
                    reason: "video too far ahead, resync at keyframe",
                });
            }
            return Ok(SyncDecision::Hold {
                until: ClockTime::new((audio_ms + self.max_drift_ms) * 1000),
            });
        }

        // Late video: drop keyframes outright; for non-keyframes either hold or
        // drop only when explicitly allowed.
        if self.audio_available && drift_ms < -self.late_threshold_ms {
            self.clock
                .add_dropped((-drift_ms).min(self.late_threshold_ms));
            if is_keyframe {
                return Ok(SyncDecision::Drop {
                    reason: "video late, drop keyframe to catch up",
                });
            }
            if self.drop_non_reference {
                return Ok(SyncDecision::Drop {
                    reason: "video late, drop non-reference frame",
                });
            }
            return Ok(SyncDecision::Hold {
                until: ClockTime::new(audio_ms * 1000),
            });
        }

        Ok(SyncDecision::Render {
            target: render_time,
        })
    }

    /// Enable dropping of non-reference frames when catching up.
    pub fn set_drop_non_reference(&mut self, enabled: bool) {
        self.drop_non_reference = enabled;
    }

    /// Return the maximum observed |drift| in milliseconds.
    pub fn max_observed_drift_ms(&self) -> i64 {
        self.max_observed_drift_ms
    }

    /// Mark the end of the stream.
    pub fn set_ended(&mut self) {
        self.clock.set_state(ClockState::Ended);
    }

    /// Borrow the underlying clock.
    pub fn clock(&self) -> &MediaClock {
        &self.clock
    }

    /// Mutable access to the underlying clock for stats/state updates.
    pub fn clock_mut(&mut self) -> &mut MediaClock {
        &mut self.clock
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_types::{MediaTime, TimeBase, Timestamp};

    fn time_ms(ms: i64) -> MediaTime {
        MediaTime::from_pts_dts(Timestamp::new(ms), Timestamp::new(ms), TimeBase::DEFAULT)
    }

    #[test]
    fn audio_drives_clock() {
        let mut sync = AvSync::new(50, 100);
        sync.feed_audio(time_ms(100), 40, StreamEpoch::new(0));
        let decision = sync
            .feed_video(time_ms(110), false, StreamEpoch::new(0))
            .unwrap();
        assert!(matches!(decision, SyncDecision::Render { .. }));
    }

    #[test]
    fn late_keyframe_dropped() {
        let mut sync = AvSync::new(50, 100);
        sync.feed_audio(time_ms(1000), 40, StreamEpoch::new(0));
        let decision = sync
            .feed_video(time_ms(0), true, StreamEpoch::new(0))
            .unwrap();
        assert!(matches!(decision, SyncDecision::Drop { .. }));
    }

    #[test]
    fn non_reference_held_by_default() {
        let mut sync = AvSync::new(50, 100);
        sync.feed_audio(time_ms(1000), 40, StreamEpoch::new(0));
        let decision = sync
            .feed_video(time_ms(0), false, StreamEpoch::new(0))
            .unwrap();
        assert!(matches!(decision, SyncDecision::Hold { .. }));
    }
}

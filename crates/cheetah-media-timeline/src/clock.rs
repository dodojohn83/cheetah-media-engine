//! Unified media clock: converts input timestamps to a monotonic internal time,
//! handles per-epoch resets/wraps, and exposes timing statistics.

use alloc::collections::BTreeMap;

use cheetah_media_types::{MediaTime, StreamEpoch, TimeBase, Timestamp};

/// Internal timebase used by the media clock: 1 MHz ticks (microseconds).
pub const INTERNAL_TIMEBASE: TimeBase = match TimeBase::new(1, 1_000_000) {
    Some(tb) => tb,
    None => unreachable!(),
};

/// A timestamp in the internal monotonic clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ClockTime(i64);

impl ClockTime {
    pub const fn new(us: i64) -> Self {
        Self(us)
    }

    pub const fn us(self) -> i64 {
        self.0
    }

    pub const fn ms(self) -> i64 {
        self.0 / 1000
    }
}

/// Timeline-level errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineError {
    /// The sample had neither PTS nor DTS.
    MissingTimestamp,
    /// The computed clock value overflowed the internal i64 range.
    Overflow,
}

/// High-level playback clock state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClockState {
    /// Filling the first buffers before rendering starts.
    #[default]
    Preroll,
    /// Normal playback.
    Playing,
    /// Dropping frames to recover latency.
    CatchUp,
    /// Starved and waiting for data.
    Rebuffering,
    /// Reached the end of the stream.
    Ended,
}

/// Running timing statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClockStats {
    /// Estimated live latency from wall clock in milliseconds.
    pub live_latency_ms: i64,
    /// Current buffer level in milliseconds.
    pub buffer_level_ms: i64,
    /// Observed jitter in milliseconds (maximum delta deviation).
    pub jitter_ms: i64,
    /// A/V drift in milliseconds (video ahead of audio is positive).
    pub drift_ms: i64,
    /// Total duration dropped to recover sync in milliseconds.
    pub dropped_duration_ms: i64,
    /// Number of discontinuity/wrap events observed.
    pub discontinuities: u64,
    /// Number of timestamp wrap events observed.
    pub wraps: u64,
}

/// Per-epoch state used to unwrap timestamps and keep the internal clock
/// monotonic within that epoch.
#[derive(Debug, Clone, Copy)]
struct EpochState {
    base_us: i64,
    /// Last raw source ticks in the original timebase.
    last_source_ticks: i64,
    /// Last rescaled timestamp in microseconds.
    last_raw_us: i64,
    last_clock_us: i64,
    last_delta_us: i64,
}

/// A monotonic media clock with epoch-aware wrap/reset handling.
///
/// All incoming timestamps are converted to microseconds in `INTERNAL_TIMEBASE`.
/// The clock keeps per-epoch offsets so that `StreamEpoch` changes (discontinuities,
/// seeks, source switches) do not violate monotonicity, while the original sample
/// timestamps are preserved in `MediaTime`.
#[derive(Debug, Default)]
pub struct MediaClock {
    epochs: BTreeMap<StreamEpoch, EpochState>,
    last_overall_clock_us: i64,
    wrap_bits: Option<u8>,
    discontinuity_threshold_us: i64,
    state: ClockState,
    stats: ClockStats,
}

impl MediaClock {
    /// Create a new clock.
    ///
    /// `wrap_bits` enables MPEG-style timestamp unwrap (commonly 33 bits for
    /// MPEG-TS/H.264). `discontinuity_threshold_us` is the maximum backward
    /// jump before treating it as a wrap or discontinuity (default 5 s).
    pub fn new(wrap_bits: Option<u8>, discontinuity_threshold_us: Option<i64>) -> Self {
        // Timestamp unwrap is only defined for 1..=62 bit counters.
        let wrap_bits = wrap_bits.filter(|b| (1..=62).contains(b));
        Self {
            epochs: BTreeMap::new(),
            last_overall_clock_us: 0,
            wrap_bits,
            discontinuity_threshold_us: discontinuity_threshold_us.unwrap_or(5_000_000).max(0),
            state: ClockState::Preroll,
            stats: ClockStats::default(),
        }
    }

    /// Feed a sample into the clock and return its monotonic render time.
    pub fn update(
        &mut self,
        time: MediaTime,
        epoch: StreamEpoch,
    ) -> Result<ClockTime, TimelineError> {
        let (source_ticks, source_base) =
            raw_ticks_and_timebase(&time).ok_or(TimelineError::MissingTimestamp)?;

        let existing = self.epochs.get(&epoch).copied();
        let mut is_discontinuity = false;
        let mut is_wrap = false;

        // Unwrap in the original timebase so the wrap boundary matches the
        // transport-layer specification (e.g. 2^33 ticks at 90 kHz for MPEG-TS).
        let (current_source_ticks, last_delta) = if let Some(state) = existing {
            let ticks = if let Some(bits) = self.wrap_bits {
                let prev = Timestamp::new(state.last_source_ticks);
                let new = Timestamp::new(source_ticks).unwrapped_around(prev, bits);
                if new.ticks() != source_ticks {
                    is_wrap = true;
                }
                new.ticks()
            } else {
                source_ticks
            };
            (ticks, state.last_delta_us)
        } else {
            (source_ticks, 0)
        };

        let current_raw_us = source_base
            .rescale_i64(current_source_ticks, INTERNAL_TIMEBASE)
            .map_err(|_| TimelineError::Overflow)?;

        let mut base_us = if let Some(state) = existing {
            let backward_us = state.last_raw_us.saturating_sub(current_raw_us);
            if backward_us > self.discontinuity_threshold_us {
                is_discontinuity = true;
                // Shift base so the next clock value is strictly greater than the
                // previous global maximum.
                self.last_overall_clock_us
                    .saturating_sub(current_raw_us)
                    .saturating_add(1)
            } else {
                state.base_us
            }
        } else {
            if self.last_overall_clock_us == 0 {
                0
            } else {
                self.last_overall_clock_us
                    .saturating_sub(current_raw_us)
                    .saturating_add(1)
            }
        };

        let mut clock_us = base_us
            .checked_add(current_raw_us)
            .ok_or(TimelineError::Overflow)?;

        let previous_clock = existing.map_or(0, |s| s.last_clock_us);

        if clock_us < previous_clock {
            base_us = previous_clock
                .saturating_sub(current_raw_us)
                .saturating_add(1);
            clock_us = base_us
                .checked_add(current_raw_us)
                .ok_or(TimelineError::Overflow)?;
        }

        if clock_us < self.last_overall_clock_us {
            base_us = self
                .last_overall_clock_us
                .saturating_sub(current_raw_us)
                .saturating_add(1);
            clock_us = base_us
                .checked_add(current_raw_us)
                .ok_or(TimelineError::Overflow)?;
        }

        let delta =
            current_raw_us.saturating_sub(existing.map_or(current_raw_us, |s| s.last_raw_us));

        if is_discontinuity {
            self.stats.discontinuities = self.stats.discontinuities.saturating_add(1);
        }
        if is_wrap {
            self.stats.wraps = self.stats.wraps.saturating_add(1);
        }

        // Update running jitter estimate: maximum absolute deviation of
        // consecutive deltas. Skip when `last_delta` is zero (first sample or
        // after a reset/wrap/discontinuity) so the metric is not seeded with
        // the first interval.
        let jitter_deviation = delta.saturating_sub(last_delta).saturating_abs();
        let threshold = self.stats.jitter_ms.saturating_mul(1000);
        if last_delta != 0 && jitter_deviation > threshold {
            self.stats.jitter_ms = jitter_deviation / 1000;
        }

        let new_state = EpochState {
            base_us,
            last_source_ticks: current_source_ticks,
            last_raw_us: current_raw_us,
            last_clock_us: clock_us,
            last_delta_us: if is_discontinuity || is_wrap {
                0
            } else {
                delta
            },
        };
        self.epochs.insert(epoch, new_state);

        if clock_us > self.last_overall_clock_us {
            self.last_overall_clock_us = clock_us;
        }

        Ok(ClockTime::new(clock_us))
    }

    /// Return the most recent monotonic clock value seen across all epochs.
    pub fn now(&self) -> ClockTime {
        ClockTime::new(self.last_overall_clock_us)
    }

    /// Set the current playback state.
    pub fn set_state(&mut self, state: ClockState) {
        self.state = state;
    }

    /// Current playback state.
    pub fn state(&self) -> ClockState {
        self.state
    }

    /// Update statistics from external measurements.
    pub fn set_stats(&mut self, live_latency_ms: i64, buffer_level_ms: i64, drift_ms: i64) {
        self.stats.live_latency_ms = live_latency_ms;
        self.stats.buffer_level_ms = buffer_level_ms;
        self.stats.drift_ms = drift_ms;
    }

    /// Record that `ms` of content was dropped to recover sync.
    pub fn add_dropped(&mut self, ms: i64) {
        self.stats.dropped_duration_ms = self.stats.dropped_duration_ms.saturating_add(ms);
    }

    /// Latest timing statistics.
    pub fn stats(&self) -> &ClockStats {
        &self.stats
    }
}

fn raw_ticks_and_timebase(time: &MediaTime) -> Option<(i64, TimeBase)> {
    let ts = time.pts.or(time.dts)?;
    Some((ts.ticks(), time.timebase))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_types::{MediaTime, StreamEpoch, TimeBase, Timestamp};

    #[test]
    fn jitter_captures_decreasing_delta() {
        let mut clock = MediaClock::new(None, Some(5_000_000));
        let tb = TimeBase::new(1, 1_000_000).unwrap();

        let sample = |ticks: i64| {
            MediaTime::new(
                Some(Timestamp::new(ticks)),
                Some(Timestamp::new(ticks)),
                None,
                tb,
            )
        };

        assert_eq!(
            clock
                .update(sample(1000), StreamEpoch::new(0))
                .unwrap()
                .us(),
            1000
        );
        assert_eq!(clock.stats().jitter_ms, 0);

        // Second sample establishes a non-zero delta.
        assert_eq!(
            clock
                .update(sample(2000), StreamEpoch::new(0))
                .unwrap()
                .us(),
            2000
        );
        assert_eq!(clock.stats().jitter_ms, 0);

        // A smaller timestamp within the discontinuity threshold produces a
        // delta of zero after saturation. The jitter estimate must still see
        // the absolute deviation from the previous delta.
        assert_eq!(
            clock
                .update(sample(1500), StreamEpoch::new(0))
                .unwrap()
                .us(),
            2001
        );
        assert_eq!(clock.stats().jitter_ms, 1);
    }

    #[test]
    fn invalid_wrap_bits_are_ignored() {
        // 0 and 63 are outside the supported unwrap range and must not cause
        // an assertion when the first timestamp is fed.
        for bits in [Some(0), Some(63)] {
            let mut clock = MediaClock::new(bits, Some(5_000_000));
            let tb = TimeBase::new(1, 1_000_000).unwrap();
            let time = MediaTime::new(
                Some(Timestamp::new(1000)),
                Some(Timestamp::new(1000)),
                None,
                tb,
            );
            assert_eq!(clock.update(time, StreamEpoch::new(0)).unwrap().us(), 1000);
        }
    }
}

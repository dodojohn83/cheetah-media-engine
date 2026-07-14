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
        Self {
            epochs: BTreeMap::new(),
            last_overall_clock_us: 0,
            wrap_bits,
            discontinuity_threshold_us: discontinuity_threshold_us.unwrap_or(5_000_000),
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
        let raw_us = time_to_us(&time).ok_or(TimelineError::MissingTimestamp)?;

        let existing = self.epochs.get(&epoch).copied();
        let mut is_discontinuity = false;
        let mut is_wrap = false;

        let (mut current_raw_us, last_delta) = match existing {
            Some(state) => (state.last_raw_us, state.last_delta_us),
            None => (0, 0),
        };

        // Apply MPEG-style unwrapping if requested.
        if let Some(bits) = self.wrap_bits {
            let prev_raw = Timestamp::new(current_raw_us);
            let new_raw = Timestamp::new(raw_us).unwrapped_around(prev_raw, bits);
            if new_raw.ticks() != raw_us {
                is_wrap = true;
                current_raw_us = new_raw.ticks();
            } else {
                current_raw_us = raw_us;
            }
        } else {
            current_raw_us = raw_us;
        }

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

        let previous_clock = if let Some(state) = existing {
            state.last_clock_us
        } else {
            0
        };

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

        let delta = current_raw_us.saturating_sub(if let Some(state) = existing {
            state.last_raw_us
        } else {
            current_raw_us
        });

        if is_discontinuity {
            self.stats.discontinuities += 1;
        }
        if is_wrap {
            self.stats.wraps += 1;
        }

        // Update running jitter estimate: maximum deviation of consecutive deltas.
        let jitter_deviation = delta.saturating_sub(last_delta).abs();
        if jitter_deviation > self.stats.jitter_ms * 1000 {
            self.stats.jitter_ms = jitter_deviation / 1000;
        }

        let new_state = EpochState {
            base_us,
            last_raw_us: current_raw_us,
            last_clock_us: clock_us,
            last_delta_us: delta,
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

fn time_to_us(time: &MediaTime) -> Option<i64> {
    let ms = time.pts_ms().or_else(|| time.dts_ms())?;
    ms.checked_mul(1000)
}

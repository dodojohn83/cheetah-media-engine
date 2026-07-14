//! Copy-budget and per-stage backpressure budgets.

use alloc::collections::BTreeMap;

use crate::MediaError;

/// Reasons for unavoidable byte copies. Each copy point is named and counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CopyReason {
    /// Fetch/WS delivery into WASM linear memory.
    NetworkToWasm,
    /// Parser reassembly across chunk boundaries.
    ParserReassembly,
    /// Demuxer to decoder input construction.
    DemuxToDecoder,
    /// Demuxer to MSE segment construction.
    DemuxToMse,
    /// Decoder output to renderer upload.
    DecoderToRenderer,
    /// Any other named copy point.
    Other(&'static str),
}

impl CopyReason {
    /// Stable name used in counters and diagnostics.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NetworkToWasm => "network_to_wasm",
            Self::ParserReassembly => "parser_reassembly",
            Self::DemuxToDecoder => "demux_to_decoder",
            Self::DemuxToMse => "demux_to_mse",
            Self::DecoderToRenderer => "decoder_to_renderer",
            Self::Other(s) => s,
        }
    }
}

/// Budget of unavoidable copies between pipeline stages.
///
/// Counters are keyed by `CopyReason` so that CI can fail when any stage
/// grows its per-sample or per-second copy cost unexpectedly.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CopyBudget {
    counters: BTreeMap<CopyReason, u64>,
    total_limit: Option<u64>,
}

impl CopyBudget {
    /// Create a budget with an optional total byte limit.
    pub const fn new(total_limit: Option<u64>) -> Self {
        Self {
            counters: BTreeMap::new(),
            total_limit,
        }
    }

    /// Record `bytes` copied for `reason`.
    pub fn record(&mut self, reason: CopyReason, bytes: u64) {
        *self.counters.entry(reason).or_insert(0) += bytes;
    }

    /// Total bytes copied across all reasons.
    pub fn total(&self) -> u64 {
        self.counters.values().sum()
    }

    /// Bytes copied for a specific reason.
    pub fn get(&self, reason: CopyReason) -> u64 {
        self.counters.get(&reason).copied().unwrap_or(0)
    }

    /// Return an error if the total copy budget is exceeded.
    pub fn check(&self) -> Result<(), MediaError> {
        if let Some(limit) = self.total_limit {
            let total = self.total();
            if total > limit {
                return Err(MediaError::ResourceLimit {
                    name: "copy_budget",
                    current: total,
                    limit,
                });
            }
        }
        Ok(())
    }
}

/// Drop policy when a stage is over its in-flight watermark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum DropPolicy {
    /// Never drop frames; backpressure is propagated upstream.
    #[default]
    Never,
    /// In live mode, stale non-key video frames may be dropped; audio and
    /// decoder reference chains are preserved.
    DropNonKeyframe,
    /// Drop the oldest in-flight item to make room for the newest.
    DropOldest,
}

/// Per-stage in-flight and watermark budget.
///
/// Every pipeline stage declares its maximum outstanding work, watermarks, and
/// drop policy so that live playback can shed stale video without losing audio
/// or decoder references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageBudget {
    /// Hard limit on items/bytes in flight.
    pub max_in_flight: usize,
    /// High watermark at which backpressure is raised.
    pub high_watermark: usize,
    /// Low watermark at which backpressure is released.
    pub low_watermark: usize,
    /// What to do when the high watermark is exceeded.
    pub drop_policy: DropPolicy,
}

impl StageBudget {
    /// Create a stage budget. Panics if watermarks are inconsistent.
    pub const fn new(
        max_in_flight: usize,
        high_watermark: usize,
        low_watermark: usize,
        drop_policy: DropPolicy,
    ) -> Self {
        assert!(
            low_watermark <= high_watermark,
            "low watermark must not exceed high watermark"
        );
        assert!(
            high_watermark <= max_in_flight,
            "high watermark must not exceed max in-flight"
        );
        Self {
            max_in_flight,
            high_watermark,
            low_watermark,
            drop_policy,
        }
    }

    /// True if `current` is at or above the high watermark.
    pub const fn is_over_high(&self, current: usize) -> bool {
        current >= self.high_watermark
    }

    /// True if `current` has fallen to or below the low watermark.
    pub const fn is_below_low(&self, current: usize) -> bool {
        current <= self.low_watermark
    }

    /// True if `current` has exceeded the hard in-flight limit.
    pub const fn is_over_max(&self, current: usize) -> bool {
        current > self.max_in_flight
    }

    /// Determine whether a newly arriving item should be admitted under the
    /// configured policy. Video is droppable in live mode when it is not a
    /// keyframe; audio and decoder references are never dropped.
    pub fn should_admit(
        &self,
        current: usize,
        is_live: bool,
        is_video: bool,
        is_keyframe: bool,
    ) -> bool {
        if current < self.max_in_flight {
            return true;
        }
        match self.drop_policy {
            DropPolicy::Never => false,
            // Always admit up to the hard limit; the caller evicts the oldest
            // in-flight item to make room.
            DropPolicy::DropOldest => current <= self.max_in_flight,
            DropPolicy::DropNonKeyframe => {
                if is_live && is_video && !is_keyframe {
                    // Drop the stale non-key video frame itself.
                    false
                } else if is_live && (!is_video || is_keyframe) {
                    // Audio and keyframes are never dropped; admit one over the
                    // limit so the caller can evict stale non-key video frames.
                    current <= self.max_in_flight
                } else {
                    false
                }
            }
        }
    }

    /// Return a `ResourceLimit` error when the hard limit is exceeded and the
    /// item cannot be dropped.
    pub const fn backpressure(&self, current: usize) -> Result<(), MediaError> {
        if current > self.max_in_flight {
            Err(MediaError::ResourceLimit {
                name: "stage_in_flight",
                current: current as u64,
                limit: self.max_in_flight as u64,
            })
        } else {
            Ok(())
        }
    }
}

impl Default for StageBudget {
    fn default() -> Self {
        Self {
            max_in_flight: 16,
            high_watermark: 12,
            low_watermark: 8,
            drop_policy: DropPolicy::Never,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_budget_records_and_checks() {
        let mut budget = CopyBudget::new(Some(100));
        budget.record(CopyReason::ParserReassembly, 30);
        budget.record(CopyReason::DemuxToDecoder, 60);
        assert_eq!(budget.total(), 90);
        assert!(budget.check().is_ok());
        budget.record(CopyReason::DecoderToRenderer, 20);
        assert_eq!(budget.total(), 110);
        assert!(budget.check().is_err());
    }

    #[test]
    fn stage_budget_watermarks_and_drop_policy() {
        let budget = StageBudget::new(16, 12, 8, DropPolicy::DropNonKeyframe);
        assert!(budget.is_over_high(12));
        assert!(!budget.is_below_low(12));
        assert!(!budget.is_over_max(16));
        assert!(budget.is_over_max(17));

        // Non-key video can be dropped in live mode once at the limit.
        assert!(!budget.should_admit(16, true, true, false));
        // Audio and keyframes are not dropped.
        assert!(budget.should_admit(16, true, false, false));
        assert!(budget.should_admit(16, true, true, true));
        // Non-live always admits until the hard limit.
        assert!(budget.should_admit(15, false, true, false));
        assert!(!budget.should_admit(16, false, true, false));

        // DropOldest admits at the limit so the caller can evict the oldest item.
        let oldest = StageBudget::new(16, 12, 8, DropPolicy::DropOldest);
        assert!(oldest.should_admit(16, false, true, false));
        assert!(oldest.should_admit(16, true, true, false));
        assert!(!oldest.should_admit(17, true, true, false));

        // Never drops: backpressure at the limit.
        let never = StageBudget::new(16, 12, 8, DropPolicy::Never);
        assert!(!never.should_admit(16, true, false, false));
    }

    #[test]
    fn stage_budget_backpressure_returns_error() {
        let budget = StageBudget::default();
        assert!(budget.backpressure(0).is_ok());
        assert!(budget.backpressure(17).is_err());
    }
}

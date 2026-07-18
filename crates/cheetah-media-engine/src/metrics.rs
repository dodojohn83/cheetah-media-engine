//! Performance and resource instrumentation.
//!
//! `Metrics` collects per-boundary copy counts/bytes, allocation counts,
//! pool hit/miss rates and peak in-flight values. The engine can snapshot these
//! into `EngineEvent::Metrics` for benchmark and soak analysis.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};

use cheetah_media_types::{CopyBudget, CopyReason, PoolStats};

/// Per-reason copy bytes and operation count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CopyMetric {
    pub bytes: u64,
    pub count: u64,
}

/// Immutable snapshot of engine performance metrics.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MetricsSnapshot {
    /// Copy bytes and counts keyed by `CopyReason::as_str()`.
    pub copy: BTreeMap<String, CopyMetric>,
    /// Total allocations observed (count and bytes).
    pub allocations: AllocationMetric,
    /// Buffer pool hit/miss counts.
    pub pool_hits: usize,
    pub pool_misses: usize,
    /// Peak number of in-flight scheduler items.
    pub peak_in_flight: usize,
    /// Current number of in-flight scheduler items.
    pub current_in_flight: usize,
    /// Total dropped milliseconds recovered by the latency controller.
    pub total_dropped_ms: i64,
}

/// Total allocation count and bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AllocationMetric {
    pub count: u64,
    pub bytes: u64,
}

/// Mutable metrics collector for the engine.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Metrics {
    copy_budget: CopyBudget,
    allocations: AllocationMetric,
    pool_hits: usize,
    pool_misses: usize,
    peak_in_flight: usize,
    current_in_flight: usize,
    total_dropped_ms: i64,
}

impl Metrics {
    /// Create an empty metrics collector.
    pub const fn new() -> Self {
        Self {
            copy_budget: CopyBudget::new(None),
            allocations: AllocationMetric { count: 0, bytes: 0 },
            pool_hits: 0,
            pool_misses: 0,
            peak_in_flight: 0,
            current_in_flight: 0,
            total_dropped_ms: 0,
        }
    }

    /// Record a copy of `bytes` for `reason`.
    pub fn record_copy(&mut self, reason: CopyReason, bytes: u64) {
        self.copy_budget.record(reason, bytes);
    }

    /// Record an allocation of `bytes`.
    pub fn record_allocation(&mut self, bytes: u64) {
        self.allocations.count = self.allocations.count.saturating_add(1);
        self.allocations.bytes = self.allocations.bytes.saturating_add(bytes);
    }

    /// Update pool hit/miss counters from a `PoolStats` snapshot.
    ///
    /// `PoolStats` already contains cumulative totals from the pool, so this
    /// assigns them directly rather than accumulating them as deltas.
    pub fn record_pool_stats(&mut self, stats: PoolStats) {
        self.pool_hits = stats.hits;
        self.pool_misses = stats.misses;
    }

    /// Update the current in-flight count and peak.
    pub fn record_in_flight(&mut self, current: usize) {
        self.current_in_flight = current;
        if current > self.peak_in_flight {
            self.peak_in_flight = current;
        }
    }

    /// Record total dropped milliseconds from the latency controller.
    pub fn record_dropped_ms(&mut self, ms: i64) {
        self.total_dropped_ms = ms;
    }

    /// Take an immutable snapshot.
    pub fn snapshot(&self) -> MetricsSnapshot {
        let mut copy = BTreeMap::new();
        for reason in [
            CopyReason::NetworkToWasm,
            CopyReason::ParserReassembly,
            CopyReason::DemuxToDecoder,
            CopyReason::DemuxToMse,
            CopyReason::DecoderToRenderer,
        ] {
            let bytes = self.copy_budget.get(reason);
            let count = self.copy_budget.get_count(reason);
            if bytes > 0 || count > 0 {
                copy.insert(reason.as_str().to_string(), CopyMetric { bytes, count });
            }
        }
        for (reason, counter) in self.copy_budget.counters().iter() {
            let key = reason.as_str().to_string();
            let entry = copy.entry(key).or_default();
            entry.bytes = counter.bytes;
            entry.count = counter.count;
        }

        MetricsSnapshot {
            copy,
            allocations: self.allocations,
            pool_hits: self.pool_hits,
            pool_misses: self.pool_misses,
            peak_in_flight: self.peak_in_flight,
            current_in_flight: self.current_in_flight,
            total_dropped_ms: self.total_dropped_ms,
        }
    }

    /// Reset all counters.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_record_copy_and_snapshot() {
        let mut m = Metrics::new();
        m.record_copy(CopyReason::ParserReassembly, 100);
        m.record_copy(CopyReason::ParserReassembly, 50);
        m.record_copy(CopyReason::NetworkToWasm, 200);
        let snapshot = m.snapshot();
        assert_eq!(snapshot.copy["parser_reassembly"].bytes, 150);
        assert_eq!(snapshot.copy["parser_reassembly"].count, 2);
        assert_eq!(snapshot.copy["network_to_wasm"].bytes, 200);
        assert_eq!(snapshot.copy["network_to_wasm"].count, 1);
    }

    #[test]
    fn metrics_track_pool_and_in_flight() {
        let mut m = Metrics::new();
        m.record_pool_stats(PoolStats {
            hits: 3,
            misses: 1,
            ..Default::default()
        });
        // Cumulative totals from the pool must not be double-counted on repeat calls.
        m.record_pool_stats(PoolStats {
            hits: 5,
            misses: 2,
            ..Default::default()
        });
        m.record_in_flight(5);
        m.record_in_flight(3);
        m.record_in_flight(8);
        let snapshot = m.snapshot();
        assert_eq!(snapshot.pool_hits, 5);
        assert_eq!(snapshot.pool_misses, 2);
        assert_eq!(snapshot.current_in_flight, 8);
        assert_eq!(snapshot.peak_in_flight, 8);
    }

    #[test]
    fn metrics_reset_clears() {
        let mut m = Metrics::new();
        m.record_copy(CopyReason::DemuxToDecoder, 10);
        m.record_allocation(1024);
        m.reset();
        let snapshot = m.snapshot();
        assert!(snapshot.copy.is_empty());
        assert_eq!(snapshot.allocations.bytes, 0);
    }
}

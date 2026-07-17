//! Per-player resource ledger.
//!
//! `ResourceLedger` tracks long-lived handles that must be released when a
//! session stops or the engine is destroyed. It is intentionally simple so it
//! can be embedded in the `no_std` engine and updated from any stage.

use alloc::vec::Vec;

use cheetah_media_types::MediaError;

/// Resource kinds that can leak across stage boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ResourceKind {
    /// Active fetch or WebSocket connection.
    Network = 0,
    /// Pending timer or interval.
    Timer = 1,
    /// Worker or thread handle.
    Worker = 2,
    /// WASM module/instance/memory handle.
    WasmHandle = 3,
    /// Decoder instance (audio or video).
    Decoder = 4,
    /// Media frame or packet holding GPU/memory data.
    Frame = 5,
    /// Audio context / AudioWorklet / ring-buffer resource.
    Audio = 6,
    /// GPU context, buffer, texture or pipeline object.
    Gpu = 7,
    /// Object URL, MediaSource blob, or revoked URL.
    Url = 8,
    /// DOM / event listener / callback registration.
    Listener = 9,
}

const KIND_COUNT: usize = 10;

const ALL_KINDS: [ResourceKind; KIND_COUNT] = [
    ResourceKind::Network,
    ResourceKind::Timer,
    ResourceKind::Worker,
    ResourceKind::WasmHandle,
    ResourceKind::Decoder,
    ResourceKind::Frame,
    ResourceKind::Audio,
    ResourceKind::Gpu,
    ResourceKind::Url,
    ResourceKind::Listener,
];

/// A resource ownership ledger with per-kind counts and a total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLedger {
    counts: [u64; KIND_COUNT],
}

impl Default for ResourceLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceLedger {
    /// Create an empty ledger.
    pub fn new() -> Self {
        Self {
            counts: [0; KIND_COUNT],
        }
    }

    pub(crate) fn idx(kind: ResourceKind) -> usize {
        kind as usize
    }

    /// Acquire one resource of `kind`.
    pub fn acquire(&mut self, kind: ResourceKind) {
        self.counts[Self::idx(kind)] = self.counts[Self::idx(kind)].saturating_add(1);
    }

    /// Release one resource of `kind`.
    ///
    /// Returns `true` if the release was balanced and `false` if it would have
    /// driven the count below zero.
    pub fn release(&mut self, kind: ResourceKind) -> bool {
        let i = Self::idx(kind);
        if self.counts[i] == 0 {
            return false;
        }
        self.counts[i] -= 1;
        true
    }

    /// Current count for `kind`.
    pub fn count(&self, kind: ResourceKind) -> u64 {
        self.counts[Self::idx(kind)]
    }

    /// Total count across all kinds.
    pub fn total(&self) -> u64 {
        self.counts
            .iter()
            .copied()
            .fold(0u64, |a, b| a.saturating_add(b))
    }

    /// Whether every kind is at zero.
    pub fn is_zero(&self) -> bool {
        self.counts.iter().all(|c| *c == 0)
    }

    /// Reset all counts to zero, returning the previous counts.
    pub fn reset(&mut self) -> [u64; KIND_COUNT] {
        let prev = self.counts;
        self.counts = [0; KIND_COUNT];
        prev
    }

    /// Kinds that currently have a non-zero count.
    pub fn open_kinds(&self) -> Vec<ResourceKind> {
        let mut out = Vec::new();
        for (i, c) in self.counts.iter().enumerate() {
            if *c > 0 {
                out.push(ALL_KINDS[i]);
            }
        }
        out
    }
}

/// Per-kind and total resource caps used by the broadcast engine.
///
/// A default instance places no effective limit on any kind (`u64::MAX`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    max_per_kind: [u64; KIND_COUNT],
    max_total: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceLimits {
    /// Create limits with all caps set to `u64::MAX`.
    pub const fn new() -> Self {
        Self {
            max_per_kind: [u64::MAX; KIND_COUNT],
            max_total: u64::MAX,
        }
    }

    /// Set the cap for a specific resource kind.
    pub fn set_max(&mut self, kind: ResourceKind, max: u64) {
        self.max_per_kind[ResourceLedger::idx(kind)] = max;
    }

    /// Set the total cap across all kinds.
    pub fn set_max_total(&mut self, max: u64) {
        self.max_total = max;
    }

    /// Return the cap for `kind`.
    pub fn max_for(&self, kind: ResourceKind) -> u64 {
        self.max_per_kind[ResourceLedger::idx(kind)]
    }

    /// Return the total cap.
    pub fn max_total(&self) -> u64 {
        self.max_total
    }

    /// Check whether `ledger` is within these limits.
    pub fn check(&self, ledger: &ResourceLedger) -> Result<(), MediaError> {
        let total = ledger.total();
        if total > self.max_total {
            return Err(MediaError::ResourceLimit {
                name: "resource_total",
                current: total,
                limit: self.max_total,
            });
        }
        for (i, kind) in ALL_KINDS.iter().enumerate() {
            let count = ledger.count(*kind);
            let limit = self.max_per_kind[i];
            if count > limit {
                return Err(MediaError::ResourceLimit {
                    name: resource_kind_name(*kind),
                    current: count,
                    limit,
                });
            }
        }
        Ok(())
    }
}

fn resource_kind_name(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Network => "network",
        ResourceKind::Timer => "timer",
        ResourceKind::Worker => "worker",
        ResourceKind::WasmHandle => "wasm_handle",
        ResourceKind::Decoder => "decoder",
        ResourceKind::Frame => "frame",
        ResourceKind::Audio => "audio",
        ResourceKind::Gpu => "gpu",
        ResourceKind::Url => "url",
        ResourceKind::Listener => "listener",
    }
}

/// RAII guard that releases a resource when dropped.
#[derive(Debug)]
pub struct ResourceGuard<'a> {
    ledger: &'a mut ResourceLedger,
    kind: ResourceKind,
    released: bool,
}

impl<'a> ResourceGuard<'a> {
    /// Acquire a resource and return a guard.
    pub fn acquire(ledger: &'a mut ResourceLedger, kind: ResourceKind) -> Self {
        ledger.acquire(kind);
        Self {
            ledger,
            kind,
            released: false,
        }
    }

    /// Keep the resource acquired after the guard drops.
    pub fn keep(mut self) {
        self.released = true;
    }
}

impl<'a> Drop for ResourceGuard<'a> {
    fn drop(&mut self) {
        if !self.released {
            self.ledger.release(self.kind);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_tracks_acquisitions_and_releases() {
        let mut ledger = ResourceLedger::new();
        ledger.acquire(ResourceKind::Network);
        ledger.acquire(ResourceKind::Network);
        ledger.acquire(ResourceKind::Gpu);
        assert_eq!(ledger.count(ResourceKind::Network), 2);
        assert_eq!(ledger.count(ResourceKind::Gpu), 1);
        assert_eq!(ledger.total(), 3);
        assert!(!ledger.is_zero());

        assert!(ledger.release(ResourceKind::Network));
        assert_eq!(ledger.count(ResourceKind::Network), 1);
    }

    #[test]
    fn release_below_zero_returns_false_and_does_not_panic() {
        let mut ledger = ResourceLedger::new();
        assert!(!ledger.release(ResourceKind::Worker));
        assert_eq!(ledger.count(ResourceKind::Worker), 0);
    }

    #[test]
    fn reset_clears_all_counts() {
        let mut ledger = ResourceLedger::new();
        ledger.acquire(ResourceKind::Timer);
        ledger.acquire(ResourceKind::Url);
        let prev = ledger.reset();
        assert_eq!(prev[ResourceKind::Timer as usize], 1);
        assert_eq!(prev[ResourceKind::Url as usize], 1);
        assert!(ledger.is_zero());
    }

    #[test]
    fn open_kinds_returns_only_nonzero() {
        let mut ledger = ResourceLedger::new();
        ledger.acquire(ResourceKind::Audio);
        assert_eq!(ledger.open_kinds(), Vec::from([ResourceKind::Audio]));
    }

    #[test]
    fn guard_releases_on_drop() {
        let mut ledger = ResourceLedger::new();
        let guard = ResourceGuard::acquire(&mut ledger, ResourceKind::Decoder);
        drop(guard);
        assert!(ledger.is_zero());
    }

    #[test]
    fn guard_keep_prevents_release() {
        let mut ledger = ResourceLedger::new();
        {
            let guard = ResourceGuard::acquire(&mut ledger, ResourceKind::Frame);
            guard.keep();
        }
        assert_eq!(ledger.count(ResourceKind::Frame), 1);
        assert!(ledger.release(ResourceKind::Frame));
        assert!(ledger.is_zero());
    }

    #[test]
    fn ledger_never_underflows() {
        let mut ledger = ResourceLedger::new();
        for _ in 0..3 {
            ledger.release(ResourceKind::Listener);
        }
        assert_eq!(ledger.count(ResourceKind::Listener), 0);
    }

    #[test]
    fn ledger_stress_acquisitions_stay_balanced() {
        let mut ledger = ResourceLedger::new();
        for i in 0..10_000 {
            let kind = match i % 4 {
                0 => ResourceKind::Network,
                1 => ResourceKind::Timer,
                2 => ResourceKind::Decoder,
                _ => ResourceKind::Gpu,
            };
            ledger.acquire(kind);
            ledger.release(kind);
        }
        assert!(ledger.is_zero());
    }

    #[test]
    fn resource_limits_default_allow_anything() {
        let limits = ResourceLimits::default();
        let mut ledger = ResourceLedger::new();
        ledger.acquire(ResourceKind::Network);
        assert!(limits.check(&ledger).is_ok());
    }

    #[test]
    fn resource_limits_reject_per_kind_excess() {
        let mut limits = ResourceLimits::new();
        limits.set_max(ResourceKind::Network, 1);
        let mut ledger = ResourceLedger::new();
        ledger.acquire(ResourceKind::Network);
        ledger.acquire(ResourceKind::Network);
        let err = limits.check(&ledger).unwrap_err();
        assert_eq!(err.code(), 5001);
    }

    #[test]
    fn resource_limits_reject_total_excess() {
        let mut limits = ResourceLimits::new();
        limits.set_max_total(1);
        let mut ledger = ResourceLedger::new();
        ledger.acquire(ResourceKind::Network);
        ledger.acquire(ResourceKind::Gpu);
        let err = limits.check(&ledger).unwrap_err();
        assert!(matches!(
            err,
            MediaError::ResourceLimit {
                name: "resource_total",
                ..
            }
        ));
    }

    #[test]
    fn ledger_total_saturates_on_overflow() {
        let mut ledger = ResourceLedger::new();
        ledger.counts = [u64::MAX; KIND_COUNT];
        assert_eq!(ledger.total(), u64::MAX);
    }
}

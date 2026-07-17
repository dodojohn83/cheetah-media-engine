//! Cheetah media engine orchestration.
//!
//! This crate contains the platform-neutral state machine, scheduler and
//! pipeline planner. Platform specifics live in `cheetah-media-web-bindings`
//! and future native bindings.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

#[cfg(feature = "bidirectional")]
pub mod broadcast;
pub mod latency;
pub mod metrics;
#[cfg(feature = "native")]
pub mod native;
pub mod recovery;
pub mod resource;
pub mod scheduler;
pub mod state;

pub use latency::{LatencyAction, LatencyBreakdown, LatencyController, LatencyTarget};
pub use metrics::{AllocationMetric, CopyMetric, Metrics, MetricsSnapshot};
pub use recovery::{
    ClassificationRule, RecoveryAction, RecoveryDecision, RecoveryPolicy, RecoveryTracker,
    RetryBudget,
};
pub use resource::{ResourceGuard, ResourceKind, ResourceLedger, ResourceLimits};
pub use scheduler::{BoundedQueue, Priority, QueueConfig, QueueName, Scheduler, SchedulerEvent};
pub use state::{
    BackendEvent, Engine, EngineCommand, EngineError, EngineEvent, EngineOutput, LoadRequest,
    NetworkEvent, PlayerState,
};

use cheetah_media_backend_api::CapabilityProbe;
use cheetah_media_types::CodecId;

/// Engine version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Resource budget for a single player instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerBudget {
    /// Maximum video decoder instances.
    pub max_video_decoders: u8,
    /// Maximum buffered milliseconds.
    pub max_buffer_ms: u32,
}

impl PlayerBudget {
    /// Default budget for desktop playback.
    pub const fn desktop() -> Self {
        Self {
            max_video_decoders: 1,
            max_buffer_ms: 3000,
        }
    }
}

/// Select the best backend for a given codec from a list of probes.
pub fn select_backend<'a>(
    codec: CodecId,
    probes: &'a [&'a dyn CapabilityProbe],
) -> Option<&'a dyn CapabilityProbe> {
    probes
        .iter()
        .copied()
        .find(|probe| probe.supports_codec(codec))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysSupported;
    impl CapabilityProbe for AlwaysSupported {
        fn name(&self) -> &str {
            "always"
        }
        fn supports_codec(&self, _codec: CodecId) -> bool {
            true
        }
    }

    struct NeverSupported;
    impl CapabilityProbe for NeverSupported {
        fn name(&self) -> &str {
            "never"
        }
        fn supports_codec(&self, _codec: CodecId) -> bool {
            false
        }
    }

    #[test]
    fn version_is_set() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn select_backend_picks_first_supported() {
        let probes: Vec<&dyn CapabilityProbe> = vec![&NeverSupported, &AlwaysSupported];
        let chosen = select_backend(CodecId::H264, &probes);
        assert!(chosen.is_some());
        assert_eq!(chosen.unwrap().name(), "always");
    }

    #[test]
    fn select_backend_returns_none_when_all_unsupported() {
        let probes: Vec<&dyn CapabilityProbe> = vec![&NeverSupported];
        assert!(select_backend(CodecId::H264, &probes).is_none());
    }
}

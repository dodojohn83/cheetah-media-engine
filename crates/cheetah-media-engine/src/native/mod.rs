//! Native player orchestration: capability negotiation, diagnostics and
//! lifecycle soak.

pub mod diagnostics;
pub mod lifecycle;
pub mod negotiator;
pub mod player;
pub mod source;

pub use diagnostics::{DiagnosticEvent, Diagnostics, DiagnosticsCounters};
pub use lifecycle::{LifecycleError, LifecycleEvent, LifecycleSoak};
pub use negotiator::{AudioTarget, BackendPlan, NegotiationError, TransportKind, VideoTarget};
pub use player::{NativePlayer, NativePlayerBuilder, NativePlayerConfig, NativePlayerError};
pub use source::MemoryByteSource;

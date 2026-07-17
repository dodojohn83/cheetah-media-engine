//! Stable media error categories used across the Cheetah media engine.

/// Trait for determining whether an error is recoverable.
pub trait Recoverability {
    /// True if the caller can reasonably continue after this error.
    fn is_recoverable(&self) -> bool;
}

/// Top-level media error categories.
///
/// Each variant carries a stable numeric `code` or structured context so that
/// Rust, ABI, WASM, and TypeScript surfaces can agree on diagnostics without
/// relying on string matching.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MediaError {
    /// Input data violates a documented contract.
    InvalidInput {
        code: u32,
        context: Option<&'static str>,
    },
    /// The requested capability is not implemented or not enabled.
    Unsupported {
        code: u32,
        context: Option<&'static str>,
    },
    /// A configured or default resource limit was exceeded.
    ResourceLimit {
        name: &'static str,
        current: u64,
        limit: u64,
    },
    /// An operation exceeded its deadline.
    Timeout { stage: &'static str },
    /// The operation was cancelled by the caller.
    Cancelled,
    /// A backend (decoder, network, container) reported a failure.
    BackendFailure {
        code: u32,
        source: Option<&'static str>,
    },
    /// An internal invariant was violated; indicates a bug.
    InternalInvariant { msg: &'static str },
}

impl Recoverability for MediaError {
    fn is_recoverable(&self) -> bool {
        match self {
            Self::InvalidInput { .. } | Self::Unsupported { .. } => true,
            Self::ResourceLimit { .. } => false,
            Self::Timeout { .. } => true,
            Self::Cancelled => true,
            Self::BackendFailure { .. } => false,
            Self::InternalInvariant { .. } => false,
        }
    }
}

impl MediaError {
    /// Stable numeric code for the error category.
    pub const fn code(&self) -> u32 {
        match self {
            Self::InvalidInput { code, .. } => *code,
            Self::Unsupported { code, .. } => *code,
            Self::ResourceLimit { .. } => 5001,
            Self::Timeout { .. } => 5002,
            Self::Cancelled => 5003,
            Self::BackendFailure { code, .. } => *code,
            Self::InternalInvariant { .. } => 9000,
        }
    }

    /// Human-readable category name for telemetry.
    pub const fn stage(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "input",
            Self::Unsupported { .. } => "capability",
            Self::ResourceLimit { .. } => "limit",
            Self::Timeout { .. } => "timeout",
            Self::Cancelled => "cancel",
            Self::BackendFailure { .. } => "backend",
            Self::InternalInvariant { .. } => "invariant",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_input_is_recoverable() {
        let e = MediaError::InvalidInput {
            code: 1000,
            context: None,
        };
        assert!(e.is_recoverable());
        assert_eq!(e.code(), 1000);
    }

    #[test]
    fn internal_invariant_is_not_recoverable() {
        let e = MediaError::InternalInvariant { msg: "bug" };
        assert!(!e.is_recoverable());
        assert_eq!(e.code(), 9000);
    }
}

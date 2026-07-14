//! Platform-neutral backend port definitions.
//!
//! Backend implementations are provided by platform-specific crates; this crate
//! only defines the trait surface so the engine can be compiled without linking
//! to DOM, Qt, JNI, or other platform APIs.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

pub use port::*;

pub mod port;

use cheetah_media_types::CodecId;

/// A platform media backend capability probe.
pub trait CapabilityProbe {
    /// Human-readable backend name.
    fn name(&self) -> &str;
    /// True if the backend can decode `codec`.
    fn supports_codec(&self, codec: CodecId) -> bool;
}

/// A compressed media source (e.g. HTTP-FLV, HLS, WebSocket-fMP4).
pub trait TransportSource: Send {
    /// Pull the next chunk of compressed bytes.
    fn read_chunk(&mut self, buf: &mut [u8]) -> Result<usize, TransportError>;
}

/// Stable transport errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    WouldBlock,
    Eof,
    Reset,
    Other(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyProbe;
    impl CapabilityProbe for DummyProbe {
        fn name(&self) -> &str {
            "dummy"
        }
        fn supports_codec(&self, _codec: CodecId) -> bool {
            true
        }
    }

    #[test]
    fn probe_can_report_support() {
        let probe = DummyProbe;
        assert!(probe.supports_codec(CodecId::H264));
    }
}

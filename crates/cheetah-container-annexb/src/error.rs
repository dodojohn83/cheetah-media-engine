//! Errors returned by the Annex-B demuxer.

use core::fmt;

/// Error returned by `AnnexBDemuxer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnexbError {
    /// The input buffer would exceed the configured limit.
    BufferExceeded { max: usize },
    /// A NAL unit exceeded the configured maximum size.
    NalTooLarge { size: usize, max: usize },
    /// The stream is malformed (e.g. a NAL header is missing or a start code
    /// appears inside what should be a NAL header).
    InvalidInput,
    /// The requested codec is not supported by this demuxer.
    UnsupportedCodec,
}

impl fmt::Display for AnnexbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferExceeded { max } => write!(f, "input buffer exceeded {} bytes", max),
            Self::NalTooLarge { size, max } => {
                write!(f, "NAL size {} exceeded maximum {} bytes", size, max)
            }
            Self::InvalidInput => write!(f, "invalid Annex-B input"),
            Self::UnsupportedCodec => write!(f, "unsupported Annex-B codec"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AnnexbError {}

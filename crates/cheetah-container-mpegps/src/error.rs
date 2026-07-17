//! Errors returned by the MPEG-PS demuxer.

use core::fmt;

/// Error returned by `MpegPsDemuxer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpegPsError {
    /// The input buffer would exceed the configured limit.
    BufferExceeded { max: usize },
    /// A PES packet exceeded the configured maximum size.
    PacketTooLarge { size: usize, max: usize },
    /// The stream is malformed (e.g. a PES start code or length is invalid).
    InvalidInput,
    /// The requested video codec is not supported by this demuxer.
    UnsupportedVideoCodec,
    /// The PES start code was not recognized.
    UnrecognizedStreamId,
    /// More data is needed to complete the current packet.
    NeedMoreData,
}

impl fmt::Display for MpegPsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferExceeded { max } => write!(f, "input buffer exceeded {} bytes", max),
            Self::PacketTooLarge { size, max } => {
                write!(f, "PES packet size {} exceeded maximum {} bytes", size, max)
            }
            Self::InvalidInput => write!(f, "invalid MPEG-PS input"),
            Self::UnsupportedVideoCodec => write!(f, "unsupported MPEG-PS video codec"),
            Self::UnrecognizedStreamId => write!(f, "unrecognized PES stream id"),
            Self::NeedMoreData => write!(f, "need more MPEG-PS data"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MpegPsError {}

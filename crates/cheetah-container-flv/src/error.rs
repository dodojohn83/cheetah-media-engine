//! FLV error types.

/// Error returned by the FLV parser, demuxer, or muxer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlvError {
    /// More input bytes are needed to complete the current FLV primitive.
    NeedMoreData,
    /// The FLV signature is not "FLV".
    InvalidSignature,
    /// The FLV file header is malformed.
    MalformedHeader,
    /// A tag header or tag body is malformed.
    MalformedTag,
    /// The codec indicated in a tag is not supported.
    UnsupportedCodec,
    /// A configured resource limit was exceeded.
    LimitExceeded,
    /// A tag timestamp or composition time is out of range.
    InvalidTimestamp,
    /// Script data / AMF is malformed or exceeded depth/size limits.
    InvalidAmf,
}

impl core::fmt::Display for FlvError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NeedMoreData => write!(f, "FLV need more data"),
            Self::InvalidSignature => write!(f, "FLV invalid signature"),
            Self::MalformedHeader => write!(f, "FLV malformed header"),
            Self::MalformedTag => write!(f, "FLV malformed tag"),
            Self::UnsupportedCodec => write!(f, "FLV unsupported codec"),
            Self::LimitExceeded => write!(f, "FLV limit exceeded"),
            Self::InvalidTimestamp => write!(f, "FLV invalid timestamp"),
            Self::InvalidAmf => write!(f, "FLV invalid AMF"),
        }
    }
}

impl From<cheetah_media_bitstream::ReadError> for FlvError {
    fn from(_: cheetah_media_bitstream::ReadError) -> Self {
        Self::NeedMoreData
    }
}

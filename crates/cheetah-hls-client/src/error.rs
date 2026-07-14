//! Errors returned by the HLS/LL-HLS client.

use alloc::string::String;

/// Errors that can occur while parsing or processing HLS playlists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HlsError {
    /// Missing `#EXTM3U` magic.
    MissingExtM3u,
    /// Malformed line or tag.
    Malformed { line: u32, context: String },
    /// An attribute value could not be parsed as the expected type.
    InvalidAttribute {
        tag: String,
        key: String,
        value: String,
    },
    /// A playlist limit was exceeded.
    LimitExceeded { limit: &'static str },
    /// An unsupported HLS version or feature.
    Unsupported { feature: String },
    /// A required tag is missing.
    MissingTag { tag: String },
    /// UTF-8 decoding failed.
    Utf8Error,
}

impl HlsError {
    pub(crate) fn malformed(line: u32, context: impl Into<String>) -> Self {
        Self::Malformed {
            line,
            context: context.into(),
        }
    }

    pub(crate) fn invalid_attr(
        tag: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self::InvalidAttribute {
            tag: tag.into(),
            key: key.into(),
            value: value.into(),
        }
    }

    pub(crate) fn missing_tag(tag: impl Into<String>) -> Self {
        Self::MissingTag { tag: tag.into() }
    }
}

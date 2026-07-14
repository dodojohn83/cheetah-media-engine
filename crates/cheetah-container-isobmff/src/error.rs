//! ISOBMFF / MP4 / fMP4 error type.

use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mp4Error {
    /// More data is needed to parse the next box or field.
    NeedMoreData,
    /// The input is structurally invalid.
    InvalidInput {
        code: u32,
        context: Option<&'static str>,
    },
    /// A bounded limit was exceeded.
    LimitExceeded { limit: &'static str },
    /// A feature or codec is not supported.
    Unsupported {
        code: u32,
        context: Option<&'static str>,
    },
}

impl Mp4Error {
    pub const fn invalid_input(code: u32, context: Option<&'static str>) -> Self {
        Self::InvalidInput { code, context }
    }

    pub const fn limit_exceeded(limit: &'static str) -> Self {
        Self::LimitExceeded { limit }
    }

    pub const fn unsupported(code: u32, context: Option<&'static str>) -> Self {
        Self::Unsupported { code, context }
    }
}

impl fmt::Display for Mp4Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NeedMoreData => write!(f, "need more data"),
            Self::InvalidInput { code, context } => {
                write!(f, "invalid input ({code})")?;
                if let Some(c) = context {
                    write!(f, ": {c}")?;
                }
                Ok(())
            }
            Self::LimitExceeded { limit } => write!(f, "limit exceeded: {limit}"),
            Self::Unsupported { code, context } => {
                write!(f, "unsupported ({code})")?;
                if let Some(c) = context {
                    write!(f, ": {c}")?;
                }
                Ok(())
            }
        }
    }
}

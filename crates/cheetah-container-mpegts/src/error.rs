//! MPEG-TS parser errors.

/// Error returned by the MPEG-TS parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TsError {
    /// Need more data; call `push` and retry.
    NeedMoreData,
    /// Packet is too short to contain a 188-byte TS packet.
    PacketTooShort,
    /// Sync byte not found or lost.
    LostSync,
    /// Input data violates the transport stream contract.
    InvalidInput {
        code: u32,
        context: Option<&'static str>,
    },
    /// A section or PES exceeded a configured size limit.
    LimitExceeded { limit: &'static str },
    /// A capability is not implemented or not enabled.
    Unsupported {
        code: u32,
        context: Option<&'static str>,
    },
}

impl TsError {
    pub const fn lost_sync() -> Self {
        Self::LostSync
    }

    pub const fn invalid_input(code: u32, context: Option<&'static str>) -> Self {
        Self::InvalidInput { code, context }
    }

    pub const fn unsupported(code: u32, context: Option<&'static str>) -> Self {
        Self::Unsupported { code, context }
    }
}

impl core::fmt::Display for TsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NeedMoreData => write!(f, "need more data"),
            Self::PacketTooShort => write!(f, "packet too short"),
            Self::LostSync => write!(f, "lost sync"),
            Self::InvalidInput { code, context } => {
                write!(f, "invalid input ({code})")?;
                if let Some(ctx) = context {
                    write!(f, ": {ctx}")?;
                }
                Ok(())
            }
            Self::LimitExceeded { limit } => write!(f, "limit exceeded: {limit}"),
            Self::Unsupported { code, context } => {
                write!(f, "unsupported ({code})")?;
                if let Some(ctx) = context {
                    write!(f, ": {ctx}")?;
                }
                Ok(())
            }
        }
    }
}

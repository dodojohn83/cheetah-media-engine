//! Stable ABI error codes.

use core::fmt;

/// Stable error code returned across the ABI boundary.
///
/// Values are fixed so that JS, native and future languages agree on the
/// meaning of an error without sharing Rust enum layout.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbiError {
    /// Capability or codec is not supported in this build/configuration.
    NotSupported = 0,
    /// Input bytes violate the expected format or contract.
    InvalidData = 1,
    /// Provided buffer is too small for the requested operation.
    BufferTooSmall = 2,
    /// Operation would block; caller should poll again.
    WouldBlock = 3,
    /// Resource has been closed or destroyed.
    Closed = 4,
    /// Slot or memory descriptor has been freed or belongs to a newer generation.
    StaleHandle = 5,
    /// Index, offset or length is outside the allowed range.
    OutOfBounds = 6,
    /// A region was released more than once.
    DoubleFree = 7,
    /// Handle was created by a different engine instance.
    WrongInstance = 8,
}

impl AbiError {
    /// Numeric code sent across the ABI boundary.
    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    /// Stable diagnostic string for the error.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotSupported => "not_supported",
            Self::InvalidData => "invalid_data",
            Self::BufferTooSmall => "buffer_too_small",
            Self::WouldBlock => "would_block",
            Self::Closed => "closed",
            Self::StaleHandle => "stale_handle",
            Self::OutOfBounds => "out_of_bounds",
            Self::DoubleFree => "double_free",
            Self::WrongInstance => "wrong_instance",
        }
    }
}

impl fmt::Display for AbiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

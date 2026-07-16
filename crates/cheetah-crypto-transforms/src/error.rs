use core::fmt;

/// Errors returned by decryption transforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoError {
    /// The provided key length did not match the algorithm's required length.
    InvalidKeyLength { expected: usize, got: usize },
    /// The provided IV length did not match the algorithm's required length.
    InvalidIvLength { expected: usize, got: usize },
    /// The input length was not a valid multiple of the cipher block size.
    InvalidInputLength,
    /// PKCS#7 padding was malformed or inconsistent.
    BadPadding,
    /// `finalize` has already been called on this transform instance.
    AlreadyFinalized,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKeyLength { expected, got } => {
                write!(
                    f,
                    "invalid key length: expected {expected} bytes, got {got}"
                )
            }
            Self::InvalidIvLength { expected, got } => {
                write!(f, "invalid IV length: expected {expected} bytes, got {got}")
            }
            Self::InvalidInputLength => f.write_str("input length is not a valid block multiple"),
            Self::BadPadding => f.write_str("invalid PKCS#7 padding"),
            Self::AlreadyFinalized => f.write_str("transform has already been finalized"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CryptoError {}

use crate::error::CryptoError;

/// A stateful, Sans-I/O decryption transform.
///
/// `update` consumes the next chunk of ciphertext and returns the plaintext
/// produced by *this* call. The returned slice is valid until the next call to
/// `update` or `finalize` on the same transform.
pub trait Transform {
    /// Process the next chunk of input.
    fn update(&mut self, data: &[u8]) -> Result<&[u8], CryptoError>;

    /// Finalize the transform and return any remaining plaintext.
    fn finalize(&mut self) -> Result<&[u8], CryptoError>;
}

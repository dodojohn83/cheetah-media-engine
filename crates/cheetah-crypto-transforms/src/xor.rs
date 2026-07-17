use alloc::vec::Vec;

use crate::{CryptoError, Transform};

/// XOR transform that XORs each input byte with a repeating key.
pub struct XorTransform {
    key: Vec<u8>,
    pos: usize,
    out: Vec<u8>,
}

impl XorTransform {
    /// Create a new XOR transform with the supplied key.
    pub fn new(key: &[u8]) -> Result<Self, CryptoError> {
        if key.is_empty() {
            return Err(CryptoError::InvalidKeyLength {
                expected: 1,
                got: 0,
            });
        }
        Ok(Self {
            key: key.to_vec(),
            pos: 0,
            out: Vec::new(),
        })
    }
}

impl Transform for XorTransform {
    fn update(&mut self, data: &[u8]) -> Result<&[u8], CryptoError> {
        self.out.clear();
        self.out.reserve(data.len());
        let key_len = self.key.len();
        for (i, &b) in data.iter().enumerate() {
            let key_byte = self.key[(self.pos + i) % key_len];
            self.out.push(b ^ key_byte);
        }
        self.pos = (self.pos + data.len()) % key_len;
        Ok(&self.out)
    }

    fn finalize(&mut self) -> Result<&[u8], CryptoError> {
        Ok(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_repeating_key() {
        let key = b"abc";
        let plaintext = b"hello world";
        let mut enc = XorTransform::new(key).unwrap();
        let ciphertext = enc.update(plaintext).unwrap().to_vec();
        let mut dec = XorTransform::new(key).unwrap();
        let decrypted = dec.update(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn rejects_empty_key() {
        assert!(matches!(
            XorTransform::new(b""),
            Err(CryptoError::InvalidKeyLength {
                expected: 1,
                got: 0
            })
        ));
    }

    #[test]
    fn incremental_xor_matches_one_shot() {
        let key = b"secret";
        let data = b"the quick brown fox jumps over the lazy dog";
        let mut full = XorTransform::new(key).unwrap();
        let expected = full.update(data).unwrap().to_vec();

        let mut incremental = XorTransform::new(key).unwrap();
        let mut output = Vec::new();
        for chunk in data.chunks(7) {
            output.extend_from_slice(incremental.update(chunk).unwrap());
        }
        assert_eq!(output, expected);
    }
}

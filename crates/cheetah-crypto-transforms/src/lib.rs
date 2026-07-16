#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]

extern crate alloc;

pub mod cbc;
pub mod error;
pub mod transform;
pub mod xor;

pub use error::CryptoError;
pub use transform::Transform;
pub use xor::XorTransform;

/// AES-128-CBC decryption transform.
pub type Aes128CbcTransform = cbc::CbcTransform<aes::Aes128>;

/// SM4-CBC decryption transform.
pub type Sm4CbcTransform = cbc::CbcTransform<sm4::Sm4>;

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;
    use core::convert::TryFrom;

    use super::*;
    use cipher::{BlockCipherEncrypt, KeyInit};

    fn pkcs7_pad(block_size: usize, data: &[u8]) -> Vec<u8> {
        let pad_len = block_size - (data.len() % block_size);
        let mut out = data.to_vec();
        out.resize(data.len() + pad_len, pad_len as u8);
        out
    }

    fn aes128_cbc_encrypt(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Vec<u8> {
        let cipher = aes::Aes128::new(<&cipher::Key<aes::Aes128>>::try_from(key).unwrap());
        let block_size = <aes::Aes128 as cipher::BlockSizeUser>::block_size();
        let padded = pkcs7_pad(block_size, plaintext);
        let mut prev: [u8; 16] = <[u8; 16] as TryFrom<&[u8]>>::try_from(iv).unwrap();
        let mut out = Vec::new();
        for block in padded.chunks(block_size) {
            let mut b: [u8; 16] = <[u8; 16] as TryFrom<&[u8]>>::try_from(block).unwrap();
            for (x, p) in b.iter_mut().zip(prev.iter()) {
                *x ^= p;
            }
            let mut block_array = cipher::Block::<aes::Aes128>::default();
            block_array.as_mut_slice().copy_from_slice(&b);
            cipher.encrypt_block(&mut block_array);
            let ct = block_array.as_slice();
            out.extend_from_slice(ct);
            prev.copy_from_slice(ct);
        }
        out
    }

    fn sm4_cbc_encrypt(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Vec<u8> {
        let cipher = sm4::Sm4::new(<&cipher::Key<sm4::Sm4>>::try_from(key).unwrap());
        let block_size = <sm4::Sm4 as cipher::BlockSizeUser>::block_size();
        let padded = pkcs7_pad(block_size, plaintext);
        let mut prev: [u8; 16] = <[u8; 16] as TryFrom<&[u8]>>::try_from(iv).unwrap();
        let mut out = Vec::new();
        for block in padded.chunks(block_size) {
            let mut b: [u8; 16] = <[u8; 16] as TryFrom<&[u8]>>::try_from(block).unwrap();
            for (x, p) in b.iter_mut().zip(prev.iter()) {
                *x ^= p;
            }
            let mut block_array = cipher::Block::<sm4::Sm4>::default();
            block_array.as_mut_slice().copy_from_slice(&b);
            cipher.encrypt_block(&mut block_array);
            let ct = block_array.as_slice();
            out.extend_from_slice(ct);
            prev.copy_from_slice(ct);
        }
        out
    }

    #[test]
    fn aes128_cbc_round_trip_in_chunks() {
        let key = b"abcdefghijklmnop";
        let iv = b"1234567890123456";
        let plaintext = b"hello, aes-128-cbc world!";
        let ciphertext = aes128_cbc_encrypt(key, iv, plaintext);

        let mut transform = Aes128CbcTransform::new(key, iv).unwrap();
        let mut out = Vec::new();
        for chunk in ciphertext.chunks(5) {
            out.extend_from_slice(transform.update(chunk).unwrap());
        }
        out.extend_from_slice(transform.finalize().unwrap());
        assert_eq!(out, plaintext);
    }

    #[test]
    fn sm4_cbc_round_trip() {
        let key = b"0123456789abcdef";
        let iv = b"fedcba9876543210";
        let plaintext = b"SM4 CBC plaintext";
        let ciphertext = sm4_cbc_encrypt(key, iv, plaintext);

        let mut transform = Sm4CbcTransform::new(key, iv).unwrap();
        let decrypted = transform.update(&ciphertext).unwrap().to_vec();
        let final_decrypted = transform.finalize().unwrap();
        let mut full = decrypted;
        full.extend_from_slice(final_decrypted);
        assert_eq!(full, plaintext);
    }

    #[test]
    fn aes128_rejects_invalid_key_or_iv() {
        assert!(Aes128CbcTransform::new(b"short", b"1234567890123456").is_err());
        assert!(Aes128CbcTransform::new(b"0123456789abcdef", b"short").is_err());
    }

    #[test]
    fn aes128_rejects_bad_padding() {
        let key = b"0123456789abcdef";
        let iv = b"1234567890123456";
        // 16 bytes, last byte claims 32 bytes of padding
        let bad = [0u8; 16];
        let mut transform = Aes128CbcTransform::new(key, iv).unwrap();
        assert!(transform.update(&bad).is_ok());
        assert!(transform.finalize().is_err());
    }

    #[test]
    fn aes128_empty_input() {
        let key = b"0123456789abcdef";
        let iv = b"1234567890123456";
        let mut transform = Aes128CbcTransform::new(key, iv).unwrap();
        assert!(transform.update(b"").unwrap().is_empty());
        assert!(transform.finalize().unwrap().is_empty());
    }

    #[test]
    fn aes128_known_vector_nist_sp800_38a() {
        use hex_literal::hex;

        // NIST SP 800-38A F.2.1 AES-128-CBC Example Vector.
        // The plaintext is one block, so PKCS#7 adds a full padding block; the
        // first ciphertext block must still match the published value.
        let key: [u8; 16] = hex!("2b7e151628aed2a6abf7158809cf4f3c");
        let iv: [u8; 16] = hex!("000102030405060708090a0b0c0d0e0f");
        let plaintext: [u8; 16] = hex!("6bc1bee22e409f96e93d7e117393172a");

        let enc_ciphertext = aes128_cbc_encrypt(&key, &iv, &plaintext);
        let expected_first_block: [u8; 16] = hex!("7649abac8119b246cee98e9b12e9197d");
        assert_eq!(&enc_ciphertext[..16], &expected_first_block[..]);

        let mut transform = Aes128CbcTransform::new(&key, &iv).unwrap();
        let mut dec = Vec::new();
        for chunk in enc_ciphertext.chunks(7) {
            dec.extend_from_slice(transform.update(chunk).unwrap());
        }
        dec.extend_from_slice(transform.finalize().unwrap());
        assert_eq!(dec, plaintext);
    }
}

use alloc::vec::Vec;

use cipher::{Block, BlockCipherDecrypt, BlockSizeUser, Key, KeyInit, typenum::Unsigned};

use crate::{CryptoError, Transform};

/// PKCS#7 unpad for a single final block. Returns the plaintext length within the
/// block, or `BadPadding` if the padding is invalid.
fn pkcs7_unpad(block: &[u8]) -> Result<usize, CryptoError> {
    let last = block.last().copied().ok_or(CryptoError::BadPadding)?;
    let pad_len = last as usize;
    if pad_len == 0 || pad_len > block.len() {
        return Err(CryptoError::BadPadding);
    }
    let start = block.len() - pad_len;
    if block[start..].iter().any(|&b| b != last) {
        return Err(CryptoError::BadPadding);
    }
    Ok(start)
}

/// CBC decryption transform for any 128-bit block cipher.
pub struct CbcTransform<C: BlockCipherDecrypt + BlockSizeUser + KeyInit> {
    cipher: C,
    iv: Block<C>,
    buf: Vec<u8>,
    out: Vec<u8>,
    finalized: bool,
}

impl<C: BlockCipherDecrypt + BlockSizeUser + KeyInit> CbcTransform<C> {
    /// Create a new CBC transform with the supplied key and IV.
    pub fn new(key: &[u8], iv: &[u8]) -> Result<Self, CryptoError> {
        let expected_key = C::KeySize::USIZE;
        let key_ref = <&Key<C>>::try_from(key).map_err(|_| CryptoError::InvalidKeyLength {
            expected: expected_key,
            got: key.len(),
        })?;

        let expected_iv = C::BlockSize::USIZE;
        let iv_ref = <&Block<C>>::try_from(iv).map_err(|_| CryptoError::InvalidIvLength {
            expected: expected_iv,
            got: iv.len(),
        })?;

        Ok(Self {
            cipher: C::new(key_ref),
            iv: iv_ref.clone(),
            buf: Vec::new(),
            out: Vec::new(),
            finalized: false,
        })
    }

    fn process_block(&mut self, ct: &[u8]) -> Result<(), CryptoError> {
        let block_size = C::BlockSize::USIZE;
        let ct_block =
            <Block<C>>::try_from(&ct[..block_size]).map_err(|_| CryptoError::InvalidInputLength)?;
        let prev_iv = core::mem::replace(&mut self.iv, ct_block.clone());

        let mut pt = ct_block;
        self.cipher.decrypt_block(&mut pt);
        for (a, b) in pt.as_mut_slice().iter_mut().zip(prev_iv.as_slice()) {
            *a ^= b;
        }
        self.out.extend_from_slice(pt.as_slice());
        Ok(())
    }
}

impl<C: BlockCipherDecrypt + BlockSizeUser + KeyInit> Transform for CbcTransform<C> {
    fn update(&mut self, data: &[u8]) -> Result<&[u8], CryptoError> {
        if self.finalized {
            return Err(CryptoError::AlreadyFinalized);
        }
        self.out.clear();
        self.buf.extend_from_slice(data);

        let block_size = C::BlockSize::USIZE;
        // We always retain the last full block; it may be the final padding block.
        let mut consumed = 0;
        while self.buf.len() - consumed >= block_size * 2 {
            let ct = <Block<C>>::try_from(&self.buf[consumed..consumed + block_size])
                .map_err(|_| CryptoError::InvalidInputLength)?;
            self.process_block(ct.as_slice())?;
            consumed += block_size;
        }
        if consumed > 0 {
            self.buf.drain(..consumed);
        }

        Ok(&self.out)
    }

    fn finalize(&mut self) -> Result<&[u8], CryptoError> {
        if self.finalized {
            return Err(CryptoError::AlreadyFinalized);
        }
        self.finalized = true;
        self.out.clear();

        let block_size = C::BlockSize::USIZE;
        if self.buf.is_empty() {
            return Ok(&self.out);
        }
        if self.buf.len() != block_size {
            return Err(CryptoError::InvalidInputLength);
        }

        let ct_block =
            <Block<C>>::try_from(&self.buf[..]).map_err(|_| CryptoError::InvalidInputLength)?;
        let prev_iv = core::mem::replace(&mut self.iv, ct_block.clone());

        let mut pt = ct_block;
        self.cipher.decrypt_block(&mut pt);
        for (a, b) in pt.as_mut_slice().iter_mut().zip(prev_iv.as_slice()) {
            *a ^= b;
        }

        let unpadded = pkcs7_unpad(pt.as_slice())?;
        self.out.extend_from_slice(&pt.as_slice()[..unpadded]);
        Ok(&self.out)
    }
}

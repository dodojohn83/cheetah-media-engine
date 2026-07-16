//! Byte and bit-level cursor for parsing media containers.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

pub mod aac;
pub mod bit;
pub mod g711;
pub mod h264;
pub mod h265;
pub mod mp3;
pub mod rbsp;
pub mod sei;

pub use rbsp::unescape_rbsp;
pub use sei::{SeiError, SeiMessage, parse_sei};

pub use aac::{AdtsHeader, AudioSpecificConfig};
pub use bit::{BitCursor, BitError};
pub use g711::{
    G711Kind, PcmFormat, decode, decode_buffer, encode, encode_buffer, encode_buffer_f32,
    encode_f32,
};
pub use h264::{H264CodecConfig, H264Error, NalUnit as H264NalUnit};
pub use h265::{
    H265CodecConfig, H265Error, NalUnit as H265NalUnit, NalUnitType as H265NalUnitType,
};
pub use mp3::{Mp3Error, Mp3Header};

/// Error returned when a read request exceeds the remaining bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    EndOfStream,
}

/// A small byte cursor that reads big-endian integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteCursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> ByteCursor<'a> {
    /// Create a new cursor over `buf`.
    pub const fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Number of bytes left to read.
    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    /// True if no more bytes can be read.
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Read a single byte.
    pub fn read_u8(&mut self) -> Result<u8, ReadError> {
        if self.pos >= self.buf.len() {
            return Err(ReadError::EndOfStream);
        }
        let b = self.buf[self.pos];
        self.pos += 1;
        Ok(b)
    }

    /// Read two bytes as a big-endian `u16`.
    pub fn read_u16_be(&mut self) -> Result<u16, ReadError> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    /// Read three bytes as a big-endian `u32`.
    pub fn read_u24_be(&mut self) -> Result<u32, ReadError> {
        let b = self.read_bytes(3)?;
        Ok(((u32::from(b[0])) << 16) | ((u32::from(b[1])) << 8) | u32::from(b[2]))
    }

    /// Read four bytes as a big-endian `u32`.
    pub fn read_u32_be(&mut self) -> Result<u32, ReadError> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Read `n` bytes without advancing.
    pub fn peek_bytes(&mut self, n: usize) -> Result<&'a [u8], ReadError> {
        if self.remaining() < n {
            return Err(ReadError::EndOfStream);
        }
        Ok(&self.buf[self.pos..self.pos + n])
    }

    /// Read `n` bytes and advance the cursor.
    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], ReadError> {
        if self.remaining() < n {
            return Err(ReadError::EndOfStream);
        }
        let start = self.pos;
        self.pos += n;
        Ok(&self.buf[start..self.pos])
    }

    /// Skip `n` bytes.
    pub fn skip(&mut self, n: usize) -> Result<(), ReadError> {
        self.read_bytes(n)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_u16_be_works() {
        let mut c = ByteCursor::new(&[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(c.read_u16_be().unwrap(), 0x0102);
        assert_eq!(c.read_u16_be().unwrap(), 0x0304);
    }

    #[test]
    fn read_u24_be_works() {
        let mut c = ByteCursor::new(&[0x00, 0x00, 0x01]);
        assert_eq!(c.read_u24_be().unwrap(), 1);
    }

    #[test]
    fn read_past_end_returns_error() {
        let mut c = ByteCursor::new(&[0x01]);
        assert_eq!(c.read_u16_be(), Err(ReadError::EndOfStream));
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn h264_split_annexb_does_not_panic(bytes in prop::collection::vec(0u8..=255, 0..2048)) {
            let _ = h264::split_annexb(&bytes);
        }

        #[test]
        fn h264_split_avcc_does_not_panic(
            (bytes, length_size) in (prop::collection::vec(0u8..=255, 0..2048), 1u8..=4u8),
        ) {
            if matches!(length_size, 1 | 2 | 4) {
                let _ = h264::split_avcc(&bytes, length_size);
            }
        }

        #[test]
        fn h265_split_annexb_does_not_panic(bytes in prop::collection::vec(0u8..=255, 0..2048)) {
            let _ = h265::split_annexb(&bytes);
        }

        #[test]
        fn aac_split_adts_does_not_panic(bytes in prop::collection::vec(0u8..=255, 0..2048)) {
            let _ = aac::split_adts(&bytes);
        }

        #[test]
        fn mp3_parse_does_not_panic(bytes in prop::collection::vec(0u8..=255, 0..16)) {
            let _ = mp3::Mp3Header::parse(&bytes);
        }
    }
}

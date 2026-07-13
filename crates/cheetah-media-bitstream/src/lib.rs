//! Byte-level cursor for parsing media containers.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

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
}

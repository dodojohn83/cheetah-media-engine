//! Bit-level cursor with exponential-Golomb support.

use core::fmt;

/// Error returned by bit-level reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitError {
    /// Not enough bits remain.
    EndOfStream,
    /// Encountered an invalid bit pattern (e.g. all-ones exp-golomb code).
    InvalidBits,
    /// Requested more bits than a `u64` can hold.
    Overflow,
}

impl fmt::Display for BitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EndOfStream => write!(f, "not enough bits"),
            Self::InvalidBits => write!(f, "invalid bit pattern"),
            Self::Overflow => write!(f, "bit read overflow"),
        }
    }
}

/// A cursor that reads bits from a byte slice in big-endian order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitCursor<'a> {
    buf: &'a [u8],
    /// Current byte index.
    pos: usize,
    /// Next bit to read inside the current byte (0 = most significant).
    bit: u8,
}

impl<'a> BitCursor<'a> {
    /// Create a new cursor over `buf`.
    pub const fn new(buf: &'a [u8]) -> Self {
        Self {
            buf,
            pos: 0,
            bit: 0,
        }
    }

    /// Number of bits left.
    pub fn remaining_bits(&self) -> usize {
        if self.pos >= self.buf.len() {
            return 0;
        }
        let bits_in_current = 8 - usize::from(self.bit);
        let remaining_bytes = self.buf.len() - self.pos - 1;
        bits_in_current + remaining_bytes * 8
    }

    /// True if no bits remain.
    pub fn is_empty(&self) -> bool {
        self.remaining_bits() == 0
    }

    /// True if the cursor is on a byte boundary.
    pub const fn is_byte_aligned(&self) -> bool {
        self.bit == 0
    }

    /// Skip to the next byte boundary, returning the skipped bits.
    pub fn skip_to_byte_alignment(&mut self) -> u8 {
        let skipped = if self.bit == 0 { 0 } else { 8 - self.bit };
        if self.bit != 0 {
            self.bit = 0;
            self.pos += 1;
        }
        skipped
    }

    /// Read up to 64 bits as a `u64`.
    pub fn read_bits(&mut self, n: usize) -> Result<u64, BitError> {
        if n == 0 {
            return Ok(0);
        }
        if n > 64 {
            return Err(BitError::Overflow);
        }
        if self.remaining_bits() < n {
            return Err(BitError::EndOfStream);
        }

        let mut value: u64 = 0;
        let mut needed = n;
        while needed > 0 {
            let available_in_byte = 8 - usize::from(self.bit);
            let take = needed.min(available_in_byte);
            let byte = self.buf[self.pos];
            let shift = available_in_byte - take;
            let mask = (1usize << take) - 1;
            let bits = (byte as usize >> shift) & mask;
            value = (value << take) | bits as u64;
            self.bit += take as u8;
            if self.bit >= 8 {
                self.bit = 0;
                self.pos += 1;
            }
            needed -= take;
        }

        Ok(value)
    }

    /// Read a single bit as bool.
    pub fn read_bool(&mut self) -> Result<bool, BitError> {
        self.read_bits(1).map(|v| v == 1)
    }

    /// Read an unsigned exponential-Golomb code (UEVC).
    pub fn read_ue(&mut self) -> Result<u64, BitError> {
        let mut leading_zeros: u32 = 0;
        while !self.read_bool()? {
            leading_zeros += 1;
            if leading_zeros > 63 {
                return Err(BitError::InvalidBits);
            }
        }
        let code = self.read_bits(leading_zeros as usize)?;
        Ok((1u64 << leading_zeros) - 1 + code)
    }

    /// Read a signed exponential-Golomb code (SEVC).
    pub fn read_se(&mut self) -> Result<i64, BitError> {
        let code = self.read_ue()?;
        let signed = code.div_ceil(2);
        if code % 2 == 0 {
            Ok(-(signed as i64))
        } else {
            Ok(signed as i64)
        }
    }

    /// Read a 32-bit unsigned integer.
    pub fn read_u32(&mut self, bits: usize) -> Result<u32, BitError> {
        if bits > 32 {
            return Err(BitError::Overflow);
        }
        self.read_bits(bits).map(|v| v as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_single_bits() {
        let mut c = BitCursor::new(&[0b1010_0000]);
        assert!(c.read_bool().unwrap());
        assert!(!c.read_bool().unwrap());
        assert!(c.read_bool().unwrap());
        assert!(!c.read_bool().unwrap());
    }

    #[test]
    fn read_exp_golomb() {
        // 1 010 011 00100 00101 -> values [0, 1, 2, 3, 4]
        let mut c = BitCursor::new(&[0xA6, 0x42, 0x80]);
        assert_eq!(c.read_ue().unwrap(), 0);
        assert_eq!(c.read_ue().unwrap(), 1);
        assert_eq!(c.read_ue().unwrap(), 2);
        assert_eq!(c.read_ue().unwrap(), 3);
        assert_eq!(c.read_ue().unwrap(), 4);
    }

    #[test]
    fn read_signed_exp_golomb() {
        // 1 010 011 00100 -> signed [0, 1, -1, 2]
        let mut c = BitCursor::new(&[0xA6, 0x40]);
        assert_eq!(c.read_se().unwrap(), 0);
        assert_eq!(c.read_se().unwrap(), 1);
        assert_eq!(c.read_se().unwrap(), -1);
        assert_eq!(c.read_se().unwrap(), 2);
    }

    #[test]
    fn skip_to_byte_alignment_only_advances_when_needed() {
        let mut c = BitCursor::new(&[0xff, 0x00]);
        assert!(c.read_bool().unwrap());
        assert_eq!(c.skip_to_byte_alignment(), 7);
        assert!(c.is_byte_aligned());
        assert_eq!(c.read_u32(8).unwrap(), 0x00);
        assert_eq!(c.skip_to_byte_alignment(), 0);
    }
}

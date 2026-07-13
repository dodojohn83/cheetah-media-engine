//! ISOBMFF / MP4 / fMP4 box parser.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

use cheetah_media_bitstream::ByteCursor;

/// Error returned by the ISOBMFF parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsobmffError {
    EndOfStream,
    MalformedBox,
}

/// Parsed ISOBMFF box header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxHeader {
    pub size: u64,
    pub box_type: [u8; 4],
    pub header_size: u32,
}

impl BoxHeader {
    /// Return the type as an ASCII 4-character code.
    pub fn box_type_str(&self) -> &str {
        core::str::from_utf8(&self.box_type).unwrap_or("????")
    }
}

/// Parse a single box header.
///
/// Supports 32-bit size and the extended `uint64` size when `size == 1`.
pub fn parse_box_header(input: &[u8]) -> Result<BoxHeader, IsobmffError> {
    let mut cursor = ByteCursor::new(input);
    let size = u64::from(
        cursor
            .read_u32_be()
            .map_err(|_| IsobmffError::EndOfStream)?,
    );
    let box_type: [u8; 4] = cursor
        .read_bytes(4)
        .map_err(|_| IsobmffError::EndOfStream)?
        .try_into()
        .map_err(|_| IsobmffError::MalformedBox)?;

    let (final_size, header_size) = if size == 1 {
        let ext_size = cursor
            .read_u64_ext_be()
            .map_err(|_| IsobmffError::EndOfStream)?;
        (ext_size, 16)
    } else {
        (size, 8)
    };

    if final_size != 0 && final_size < u64::from(header_size) {
        return Err(IsobmffError::MalformedBox);
    }

    Ok(BoxHeader {
        size: final_size,
        box_type,
        header_size,
    })
}

trait ReadExt {
    fn read_u64_ext_be(&mut self) -> Result<u64, cheetah_media_bitstream::ReadError>;
}

impl<'a> ReadExt for ByteCursor<'a> {
    fn read_u64_ext_be(&mut self) -> Result<u64, cheetah_media_bitstream::ReadError> {
        let b = self.read_bytes(8)?;
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(b);
        Ok(u64::from_be_bytes(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_small_box() {
        let buf = [
            0x00, 0x00, 0x00, 0x08, // size = 8
            b'f', b't', b'y', b'p', // type = ftyp
        ];
        let header = parse_box_header(&buf).unwrap();
        assert_eq!(header.size, 8);
        assert_eq!(header.box_type_str(), "ftyp");
        assert_eq!(header.header_size, 8);
    }

    #[test]
    fn parse_extended_size_box() {
        let mut buf = [0u8; 16];
        buf[0..4].copy_from_slice(&[0x00, 0x00, 0x00, 0x01]); // size = 1
        buf[4..8].copy_from_slice(b"mdat");
        buf[8..16].copy_from_slice(&16u64.to_be_bytes()); // ext size = 16
        let header = parse_box_header(&buf).unwrap();
        assert_eq!(header.size, 16);
        assert_eq!(header.box_type_str(), "mdat");
        assert_eq!(header.header_size, 16);
    }
}

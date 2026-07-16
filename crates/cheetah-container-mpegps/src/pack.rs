//! MPEG-2 Program Stream pack header parsing.

use crate::MpegPsError;

/// Minimum fixed pack header size in bytes (start code + 10 fixed bytes).
const PACK_FIXED_SIZE: usize = 14;

/// A parsed MPEG-2 PS pack header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackHeader {
    /// Byte offset in the input where the first byte after this pack header is.
    pub end_offset: usize,
}

/// Parse a pack header that begins at `data[0]` with the `0x000001BA` start code.
///
/// The fixed pack header is 14 bytes; the last byte's lower three bits hold the
/// `pack_stuffing_length`. Any stuffing bytes are skipped.
pub fn parse_pack_header(data: &[u8]) -> Result<PackHeader, MpegPsError> {
    if data.len() < PACK_FIXED_SIZE {
        return Err(MpegPsError::NeedMoreData);
    }
    if data[0] != 0x00 || data[1] != 0x00 || data[2] != 0x01 || data[3] != 0xBA {
        return Err(MpegPsError::InvalidInput);
    }

    let stuffing_len = (data[13] & 0x07) as usize;
    let end_offset = PACK_FIXED_SIZE + stuffing_len;
    if data.len() < end_offset {
        return Err(MpegPsError::NeedMoreData);
    }

    Ok(PackHeader { end_offset })
}

/// True if `code` is a pack start code identifier.
pub const fn is_pack_start_code(code: u8) -> bool {
    code == 0xBA
}

/// True if `code` is a system-level start code that should be skipped rather
/// than treated as a PES packet.
pub const fn is_system_start_code(code: u8) -> bool {
    matches!(code, 0xBB | 0xBC | 0xBE | 0xBF)
}

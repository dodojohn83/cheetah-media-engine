//! Low-level ISOBMFF box reading helpers.

use alloc::vec::Vec;
use cheetah_media_bitstream::ByteCursor;

use crate::Mp4Error;

/// Maximum nesting depth for boxes.
pub const MAX_BOX_DEPTH: usize = 16;
/// Maximum box size in bytes.
pub const MAX_BOX_SIZE: u64 = 1u64 << 40;
/// Maximum number of entries in tables.
pub const MAX_TABLE_ENTRIES: usize = 1_000_000;

/// A parsed ISO base media file format box header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxHeader {
    /// Absolute byte offset of the first byte of the box in the stream.
    pub start_offset: u64,
    /// Total size of the box, including the header.
    pub size: u64,
    /// Four-character box type as a big-endian `u32`.
    pub box_type: u32,
    /// Number of bytes occupied by the header (8 or 16).
    pub header_size: u8,
}

impl BoxHeader {
    /// Parse a box header from `data` starting at stream offset `start_offset`.
    ///
    /// Returns `Err(Mp4Error::NeedMoreData)` if `data` is too short for the header.
    /// Returns `Err(Mp4Error::InvalidInput)` for zero-sized, oversized, or malformed headers.
    pub fn parse(data: &[u8], start_offset: u64) -> Result<Self, Mp4Error> {
        if data.len() < 8 {
            return Err(Mp4Error::NeedMoreData);
        }
        let mut cursor = ByteCursor::new(data);
        let size = cursor.read_u32_be().map_err(|_| Mp4Error::NeedMoreData)? as u64;
        let box_type = cursor.read_u32_be().map_err(|_| Mp4Error::NeedMoreData)?;
        let (size, header_size) = if size == 1 {
            if data.len() < 16 {
                return Err(Mp4Error::NeedMoreData);
            }
            let size_bytes = cursor.read_bytes(8).map_err(|_| Mp4Error::NeedMoreData)?;
            let size = u64::from_be_bytes([
                size_bytes[0],
                size_bytes[1],
                size_bytes[2],
                size_bytes[3],
                size_bytes[4],
                size_bytes[5],
                size_bytes[6],
                size_bytes[7],
            ]);
            if size == 0 {
                return Err(Mp4Error::invalid_input(
                    3001,
                    Some("invalid extended box size 0"),
                ));
            }
            (size, 16)
        } else if size == 0 {
            // Box extends to end of stream; unsupported for now.
            return Err(Mp4Error::unsupported(
                3002,
                Some("open-ended box size not supported"),
            ));
        } else {
            (size, 8)
        };
        if size > MAX_BOX_SIZE {
            return Err(Mp4Error::LimitExceeded { limit: "box size" });
        }
        if size < header_size as u64 {
            return Err(Mp4Error::invalid_input(
                3003,
                Some("box size smaller than header"),
            ));
        }
        Ok(Self {
            start_offset,
            size,
            box_type,
            header_size,
        })
    }

    /// Byte offset of the box body (after the header).
    pub const fn body_offset(&self) -> u64 {
        self.start_offset + self.header_size as u64
    }

    /// Body length in bytes.
    pub fn body_len(&self) -> u64 {
        self.size - self.header_size as u64
    }
}

/// Simple byte reader for big-endian multi-byte fields.
#[derive(Debug, Clone, Copy)]
pub struct Mp4Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Mp4Cursor<'a> {
    pub const fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    fn need(&self, n: usize) -> Result<(), Mp4Error> {
        if self.remaining() < n {
            Err(Mp4Error::NeedMoreData)
        } else {
            Ok(())
        }
    }

    pub fn read_u8(&mut self) -> Result<u8, Mp4Error> {
        self.need(1)?;
        let b = self.buf[self.pos];
        self.pos += 1;
        Ok(b)
    }

    pub fn read_u16(&mut self) -> Result<u16, Mp4Error> {
        self.need(2)?;
        let v = u16::from_be_bytes([self.buf[self.pos], self.buf[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    pub fn read_u24(&mut self) -> Result<u32, Mp4Error> {
        self.need(3)?;
        let v = (u32::from(self.buf[self.pos]) << 16)
            | (u32::from(self.buf[self.pos + 1]) << 8)
            | u32::from(self.buf[self.pos + 2]);
        self.pos += 3;
        Ok(v)
    }

    pub fn read_u32(&mut self) -> Result<u32, Mp4Error> {
        self.need(4)?;
        let v = u32::from_be_bytes([
            self.buf[self.pos],
            self.buf[self.pos + 1],
            self.buf[self.pos + 2],
            self.buf[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    pub fn read_u64(&mut self) -> Result<u64, Mp4Error> {
        self.need(8)?;
        let mut v = 0u64;
        for i in 0..8 {
            v = (v << 8) | u64::from(self.buf[self.pos + i]);
        }
        self.pos += 8;
        Ok(v)
    }

    pub fn read_i32(&mut self) -> Result<i32, Mp4Error> {
        self.need(4)?;
        let v = i32::from_be_bytes([
            self.buf[self.pos],
            self.buf[self.pos + 1],
            self.buf[self.pos + 2],
            self.buf[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    pub fn read_i64(&mut self) -> Result<i64, Mp4Error> {
        self.need(8)?;
        let v = i64::from_be_bytes([
            self.buf[self.pos],
            self.buf[self.pos + 1],
            self.buf[self.pos + 2],
            self.buf[self.pos + 3],
            self.buf[self.pos + 4],
            self.buf[self.pos + 5],
            self.buf[self.pos + 6],
            self.buf[self.pos + 7],
        ]);
        self.pos += 8;
        Ok(v)
    }

    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], Mp4Error> {
        self.need(n)?;
        let start = self.pos;
        self.pos += n;
        Ok(&self.buf[start..self.pos])
    }

    pub fn skip(&mut self, n: usize) -> Result<(), Mp4Error> {
        self.need(n)?;
        self.pos += n;
        Ok(())
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn rest(&self) -> &'a [u8] {
        &self.buf[self.pos..]
    }
}

/// Convert a four-character code to a `u32` in big-endian order.
pub const fn u32_from_fourcc(fourcc: &[u8; 4]) -> u32 {
    ((fourcc[0] as u32) << 24)
        | ((fourcc[1] as u32) << 16)
        | ((fourcc[2] as u32) << 8)
        | (fourcc[3] as u32)
}

/// Convert a `u32` fourcc to a 4-byte array.
pub fn fourcc_from_u32(v: u32) -> [u8; 4] {
    [
        ((v >> 24) & 0xff) as u8,
        ((v >> 16) & 0xff) as u8,
        ((v >> 8) & 0xff) as u8,
        (v & 0xff) as u8,
    ]
}

/// Box type constants.
pub mod types {
    use super::u32_from_fourcc;

    pub const FTYP: u32 = u32_from_fourcc(b"ftyp");
    pub const MOOV: u32 = u32_from_fourcc(b"moov");
    pub const MVHD: u32 = u32_from_fourcc(b"mvhd");
    pub const TRAK: u32 = u32_from_fourcc(b"trak");
    pub const TKHD: u32 = u32_from_fourcc(b"tkhd");
    pub const MDIA: u32 = u32_from_fourcc(b"mdia");
    pub const MDHD: u32 = u32_from_fourcc(b"mdhd");
    pub const HDLR: u32 = u32_from_fourcc(b"hdlr");
    pub const MINF: u32 = u32_from_fourcc(b"minf");
    pub const VMHD: u32 = u32_from_fourcc(b"vmhd");
    pub const SMHD: u32 = u32_from_fourcc(b"smhd");
    pub const DINF: u32 = u32_from_fourcc(b"dinf");
    pub const STBL: u32 = u32_from_fourcc(b"stbl");
    pub const STSD: u32 = u32_from_fourcc(b"stsd");
    pub const STTS: u32 = u32_from_fourcc(b"stts");
    pub const CTTS: u32 = u32_from_fourcc(b"ctts");
    pub const STSC: u32 = u32_from_fourcc(b"stsc");
    pub const STSZ: u32 = u32_from_fourcc(b"stsz");
    pub const STCO: u32 = u32_from_fourcc(b"stco");
    pub const CO64: u32 = u32_from_fourcc(b"co64");
    pub const STSS: u32 = u32_from_fourcc(b"stss");
    pub const MVEX: u32 = u32_from_fourcc(b"mvex");
    pub const TREX: u32 = u32_from_fourcc(b"trex");
    pub const MEHD: u32 = u32_from_fourcc(b"mehd");
    pub const MOOF: u32 = u32_from_fourcc(b"moof");
    pub const TRAF: u32 = u32_from_fourcc(b"traf");
    pub const TFHD: u32 = u32_from_fourcc(b"tfhd");
    pub const TFDT: u32 = u32_from_fourcc(b"tfdt");
    pub const TRUN: u32 = u32_from_fourcc(b"trun");
    pub const MDAT: u32 = u32_from_fourcc(b"mdat");
    pub const AVC1: u32 = u32_from_fourcc(b"avc1");
    pub const AVC3: u32 = u32_from_fourcc(b"avc3");
    pub const HVC1: u32 = u32_from_fourcc(b"hvc1");
    pub const HEV1: u32 = u32_from_fourcc(b"hev1");
    pub const MP4A: u32 = u32_from_fourcc(b"mp4a");
    pub const AVCC: u32 = u32_from_fourcc(b"avcC");
    pub const HVCC: u32 = u32_from_fourcc(b"hvcC");
    pub const ESDS: u32 = u32_from_fourcc(b"esds");
    pub const FREE: u32 = u32_from_fourcc(b"free");
    pub const SKIP: u32 = u32_from_fourcc(b"skip");
    pub const UUID: u32 = u32_from_fourcc(b"uuid");
    pub const URL: u32 = u32_from_fourcc(b"url ");
    pub const DREF: u32 = u32_from_fourcc(b"dref");
    pub const MFHD: u32 = u32_from_fourcc(b"mfhd");
}

/// Iterate over child boxes inside `parent_data`.
pub fn iter_boxes<'a>(
    parent_data: &'a [u8],
    parent_offset: u64,
    max_depth: usize,
) -> Result<impl Iterator<Item = Result<(BoxHeader, &'a [u8]), Mp4Error>> + 'a, Mp4Error> {
    if max_depth == 0 {
        return Err(Mp4Error::LimitExceeded { limit: "box depth" });
    }
    let mut offset: usize = 0;
    let mut current_offset = parent_offset;
    Ok(core::iter::from_fn(move || {
        if offset >= parent_data.len() {
            return None;
        }
        let header = match BoxHeader::parse(&parent_data[offset..], current_offset) {
            Ok(h) => h,
            Err(e) => return Some(Err(e)),
        };
        let body_len = header.body_len() as usize;
        if offset + header.header_size as usize + body_len > parent_data.len() {
            return Some(Err(Mp4Error::NeedMoreData));
        }
        let box_start = offset + header.header_size as usize;
        let box_end = box_start + body_len;
        let slice = &parent_data[box_start..box_end];
        offset = box_end;
        current_offset += header.size;
        Some(Ok((header, slice)))
    }))
}

/// Read a full box version/flags header.
pub fn read_fullbox_header(data: &[u8]) -> Result<(u8, u32, &[u8]), Mp4Error> {
    if data.len() < 4 {
        return Err(Mp4Error::NeedMoreData);
    }
    let version = data[0];
    let flags = u32::from(data[1]) << 16 | u32::from(data[2]) << 8 | u32::from(data[3]);
    Ok((version, flags, &data[4..]))
}

/// Build a 4-byte version/flags fullbox header.
pub fn write_fullbox(version: u8, flags: u32) -> [u8; 4] {
    [
        version,
        ((flags >> 16) & 0xff) as u8,
        ((flags >> 8) & 0xff) as u8,
        (flags & 0xff) as u8,
    ]
}

/// Append a big-endian `u32` value to a `Vec<u8>`.
pub fn write_u32(v: &mut Vec<u8>, value: u32) {
    v.extend_from_slice(&value.to_be_bytes());
}

/// Append a big-endian `u64` value to a `Vec<u8>`.
pub fn write_u64(v: &mut Vec<u8>, value: u64) {
    v.extend_from_slice(&value.to_be_bytes());
}

/// Append a big-endian `u24` value to a `Vec<u8>`.
pub fn write_u24(v: &mut Vec<u8>, value: u32) {
    v.extend_from_slice(&[
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    ]);
}

/// Append a big-endian `u16` value to a `Vec<u8>`.
pub fn write_u16(v: &mut Vec<u8>, value: u16) {
    v.extend_from_slice(&value.to_be_bytes());
}

/// Build an ISOBMFF box from `box_type` and body bytes, prepending size/type.
/// Falls back to a 64-bit sized box if the body is too large for a 32-bit size.
pub fn write_box(box_type: u32, body: &[u8]) -> Vec<u8> {
    if let Some(size) = body
        .len()
        .checked_add(8)
        .and_then(|s| u32::try_from(s).ok())
    {
        let mut out = Vec::with_capacity(size as usize);
        write_u32(&mut out, size);
        write_u32(&mut out, box_type);
        out.extend_from_slice(body);
        out
    } else {
        write_box64(box_type, body)
    }
}

/// Build an ISOBMFF box with 64-bit size.
pub fn write_box64(box_type: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + body.len());
    write_u32(&mut out, 1);
    write_u32(&mut out, box_type);
    write_u64(&mut out, (16 + body.len()) as u64);
    out.extend_from_slice(body);
    out
}

/// Build a fullbox (version/flags + body) and wrap it in a typed box.
pub fn write_fullbox_box(box_type: u32, version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let mut inner = Vec::with_capacity(4 + body.len());
    inner.extend_from_slice(&write_fullbox(version, flags));
    inner.extend_from_slice(body);
    write_box(box_type, &inner)
}

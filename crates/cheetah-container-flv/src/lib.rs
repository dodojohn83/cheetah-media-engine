//! FLV container format parser.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

use cheetah_media_bitstream::ByteCursor;
use cheetah_media_types::{MediaTime, TimeBase, TrackKind};

/// Error returned by the FLV parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlvError {
    InvalidSignature,
    MalformedHeader,
    EndOfStream,
}

/// FLV content type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagType {
    Audio,
    Video,
    Script,
}

impl TagType {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            8 => Some(Self::Audio),
            9 => Some(Self::Video),
            18 => Some(Self::Script),
            _ => None,
        }
    }

    /// Map a tag type to the track kind it carries, when known.
    pub fn track_kind(self) -> Option<TrackKind> {
        match self {
            Self::Audio => Some(TrackKind::Audio),
            Self::Video => Some(TrackKind::Video),
            Self::Script => None,
        }
    }
}

/// Parsed FLV file header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlvHeader {
    pub version: u8,
    pub has_audio: bool,
    pub has_video: bool,
    pub header_size: u32,
}

/// Parsed FLV tag header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlvTagHeader {
    pub tag_type: TagType,
    pub filter: bool,
    pub data_size: u32,
    pub timestamp: MediaTime,
    pub stream_id: u32,
}

/// Parse an FLV file header.
///
/// Consumes the 9-byte header and the first 4-byte PreviousTagSize0.
pub fn parse_header(input: &[u8]) -> Result<FlvHeader, FlvError> {
    let mut cursor = ByteCursor::new(input);
    let sig = cursor.read_bytes(3).map_err(|_| FlvError::EndOfStream)?;
    if sig != b"FLV" {
        return Err(FlvError::InvalidSignature);
    }
    let version = cursor.read_u8().map_err(|_| FlvError::EndOfStream)?;
    let flags = cursor.read_u8().map_err(|_| FlvError::EndOfStream)?;
    let header_size = cursor.read_u32_be().map_err(|_| FlvError::EndOfStream)?;
    if header_size < 9 {
        return Err(FlvError::MalformedHeader);
    }

    // Skip the first PreviousTagSize0 if present.
    let _ = cursor.read_u32_be();

    Ok(FlvHeader {
        version,
        has_audio: (flags & 0x04) != 0,
        has_video: (flags & 0x01) != 0,
        header_size,
    })
}

/// Parse an FLV tag header.
pub fn parse_tag_header(input: &[u8]) -> Result<FlvTagHeader, FlvError> {
    let mut cursor = ByteCursor::new(input);
    let tag_byte = cursor.read_u8().map_err(|_| FlvError::EndOfStream)?;
    let tag_type = TagType::from_u8(tag_byte & 0x1F).ok_or(FlvError::MalformedHeader)?;
    let filter = (tag_byte & 0x20) != 0;
    let data_size = cursor.read_u24_be().map_err(|_| FlvError::EndOfStream)?;
    let ts_lower = cursor.read_u24_be().map_err(|_| FlvError::EndOfStream)?;
    let ts_extended = cursor.read_u8().map_err(|_| FlvError::EndOfStream)?;
    let timestamp_ms = (u32::from(ts_extended) << 24) | ts_lower;
    let stream_id = cursor.read_u24_be().map_err(|_| FlvError::EndOfStream)?;

    Ok(FlvTagHeader {
        tag_type,
        filter,
        data_size,
        timestamp: MediaTime::from_ticks(
            Some(i64::from(timestamp_ms)),
            Some(i64::from(timestamp_ms)),
            None,
            TimeBase::DEFAULT,
        ),
        stream_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLV_HEADER: &[u8] = b"FLV\x01\x05\x00\x00\x00\x09\x00\x00\x00\x00";

    #[test]
    fn parse_header_ok() {
        let header = parse_header(FLV_HEADER).expect("valid header");
        assert_eq!(header.version, 1);
        assert!(header.has_audio);
        assert!(header.has_video);
        assert_eq!(header.header_size, 9);
    }

    #[test]
    fn parse_header_invalid_signature() {
        assert_eq!(
            parse_header(b"FLA\x01\x05\x00\x00\x00\x09"),
            Err(FlvError::InvalidSignature)
        );
    }

    #[test]
    fn parse_tag_header_ok() {
        let mut buf = [0u8; 11];
        buf[0] = 9; // video
        buf[1] = 0;
        buf[2] = 0;
        buf[3] = 10; // data_size = 10
        buf[4] = 0;
        buf[5] = 0;
        buf[6] = 0x20; // timestamp lower = 0x20
        buf[7] = 0x00; // timestamp extended
        buf[8] = 0;
        buf[9] = 0;
        buf[10] = 0;
        let tag = parse_tag_header(&buf).expect("valid tag header");
        assert_eq!(tag.tag_type, TagType::Video);
        assert!(!tag.filter);
        assert_eq!(tag.data_size, 10);
        assert_eq!(tag.timestamp.pts_ms(), Some(32));
    }

    #[test]
    fn parse_tag_header_with_filter_bit() {
        let mut buf = [0u8; 11];
        buf[0] = 0x29; // video (0x09) with filter bit (0x20) set
        buf[1] = 0;
        buf[2] = 0;
        buf[3] = 10;
        buf[4] = 0;
        buf[5] = 0;
        buf[6] = 0x20;
        buf[7] = 0x00;
        buf[8] = 0;
        buf[9] = 0;
        buf[10] = 0;
        let tag = parse_tag_header(&buf).expect("valid tag header");
        assert_eq!(tag.tag_type, TagType::Video);
        assert!(tag.filter);
    }
}

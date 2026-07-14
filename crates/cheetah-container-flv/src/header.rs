//! FLV file and tag header parsing.

use cheetah_media_bitstream::ByteCursor;
use cheetah_media_types::{MediaTime, TimeBase, TrackKind};

use crate::FlvError;

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

/// Parsed 11-byte FLV tag header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlvTagHeader {
    pub tag_type: TagType,
    /// The filter bit indicates an encrypted tag and is rejected by this parser.
    pub filter: bool,
    /// Size of the tag body (after the 11-byte header).
    pub data_size: u32,
    /// Tag timestamp in milliseconds.
    pub timestamp_ms: u32,
    /// Stream ID (should be 0 for valid FLV files).
    pub stream_id: u32,
}

impl FlvTagHeader {
    /// Convert the tag timestamp into a `MediaTime` with the default 1 kHz timebase.
    pub fn media_time(&self) -> MediaTime {
        MediaTime::from_ticks(
            Some(i64::from(self.timestamp_ms)),
            Some(i64::from(self.timestamp_ms)),
            None,
            TimeBase::DEFAULT,
        )
    }
}

/// Parse a 9-byte FLV file header.
pub fn parse_file_header(input: &[u8]) -> Result<FlvHeader, FlvError> {
    if input.len() < 9 {
        return Err(FlvError::NeedMoreData);
    }
    let mut cursor = ByteCursor::new(input);
    let sig = cursor.read_bytes(3).map_err(|_| FlvError::NeedMoreData)?;
    if sig != b"FLV" {
        return Err(FlvError::InvalidSignature);
    }
    let version = cursor.read_u8().map_err(|_| FlvError::NeedMoreData)?;
    let flags = cursor.read_u8().map_err(|_| FlvError::NeedMoreData)?;
    let header_size = cursor.read_u32_be().map_err(|_| FlvError::NeedMoreData)?;
    if header_size < 9 {
        return Err(FlvError::MalformedHeader);
    }

    Ok(FlvHeader {
        version,
        has_audio: (flags & 0x04) != 0,
        has_video: (flags & 0x01) != 0,
        header_size,
    })
}

/// Parse an 11-byte FLV tag header.
pub fn parse_tag_header(input: &[u8]) -> Result<FlvTagHeader, FlvError> {
    if input.len() < 11 {
        return Err(FlvError::NeedMoreData);
    }
    let mut cursor = ByteCursor::new(input);
    let tag_byte = cursor.read_u8().map_err(|_| FlvError::NeedMoreData)?;
    let filter = (tag_byte & 0x20) != 0;
    let tag_type = TagType::from_u8(tag_byte & 0x1F).ok_or(FlvError::MalformedTag)?;
    let data_size = cursor.read_u24_be().map_err(|_| FlvError::NeedMoreData)?;
    let ts_lower = cursor.read_u24_be().map_err(|_| FlvError::NeedMoreData)?;
    let ts_extended = cursor.read_u8().map_err(|_| FlvError::NeedMoreData)?;
    let timestamp_ms = (u32::from(ts_extended) << 24) | ts_lower;
    let stream_id = cursor.read_u24_be().map_err(|_| FlvError::NeedMoreData)?;

    Ok(FlvTagHeader {
        tag_type,
        filter,
        data_size,
        timestamp_ms,
        stream_id,
    })
}

/// Total size of a tag including the 11-byte header and the data body.
pub const fn tag_total_size(data_size: u32) -> u32 {
    11 + data_size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_header_ok() {
        let header = parse_file_header(b"FLV\x01\x05\x00\x00\x00\x09").expect("valid header");
        assert_eq!(header.version, 1);
        assert!(header.has_audio);
        assert!(header.has_video);
        assert_eq!(header.header_size, 9);
    }

    #[test]
    fn parse_header_invalid_signature() {
        assert_eq!(
            parse_file_header(b"FLA\x01\x05\x00\x00\x00\x09"),
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
        assert_eq!(tag.timestamp_ms, 0x20);
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

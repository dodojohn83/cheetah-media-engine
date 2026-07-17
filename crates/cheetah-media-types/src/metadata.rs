//! Side-car metadata extracted from streams: SEI messages, PES private data,
//! and other out-of-band overlays.

use alloc::vec::Vec;

/// Source of a metadata item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MetadataSource {
    /// H.264/H.265 Supplemental Enhancement Information message.
    Sei,
    /// MPEG-TS PES private data or private stream payload.
    PesPrivate,
    /// Explicitly injected by an external caller (e.g. a server overlay).
    External,
}

/// A single metadata payload extracted from the stream.
///
/// The interpretation of `key` depends on `source`:
/// - `Sei`: the SEI `payloadType`.
/// - `PesPrivate`: the PES `stream_id` when the payload came from a private stream,
///   or `0` when it came from the `PES_private_data` header field.
/// - `External`: caller-defined type tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataItem {
    pub source: MetadataSource,
    pub key: u32,
    pub value: Vec<u8>,
    /// Optional presentation timestamp in milliseconds.
    pub timestamp_ms: Option<i64>,
}

impl MetadataItem {
    /// Create a new SEI metadata item.
    pub fn sei(payload_type: u32, value: Vec<u8>) -> Self {
        Self {
            source: MetadataSource::Sei,
            key: payload_type,
            value,
            timestamp_ms: None,
        }
    }

    /// Create a new PES private-data item.
    pub fn pes_private(stream_id_or_tag: u32, value: Vec<u8>) -> Self {
        Self {
            source: MetadataSource::PesPrivate,
            key: stream_id_or_tag,
            value,
            timestamp_ms: None,
        }
    }

    /// Create a new externally-provided item.
    pub fn external(tag: u32, value: Vec<u8>) -> Self {
        Self {
            source: MetadataSource::External,
            key: tag,
            value,
            timestamp_ms: None,
        }
    }

    /// Attach a timestamp to this item.
    pub fn with_timestamp(mut self, timestamp_ms: i64) -> Self {
        self.timestamp_ms = Some(timestamp_ms);
        self
    }
}

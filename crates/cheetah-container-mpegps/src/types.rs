//! Public types for the MPEG-2 Program Stream demuxer.

use cheetah_media_types::{CodecId, MediaPacket, TrackInfo};

/// Default maximum input buffer size in bytes.
pub(crate) const DEFAULT_MAX_BUFFER_SIZE: usize = 32 * 1024 * 1024;

/// Default maximum NAL size emitted by the video ES assembler.
pub(crate) const DEFAULT_MAX_NAL_SIZE: usize = 16 * 1024 * 1024;

/// Video track identifier used for all emitted video packets.
pub(crate) const VIDEO_TRACK_ID: u32 = 1;

/// Audio track identifier used for all emitted audio packets.
pub(crate) const AUDIO_TRACK_ID: u32 = 2;

/// Configuration for the MPEG-PS demuxer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MpegPsConfig {
    /// Expected video codec: `H264` or `H265`.
    pub video_codec: CodecId,
    /// Maximum accepted PES packet size in bytes.
    pub max_packet_size_bytes: usize,
    /// Maximum internal buffer size in bytes.
    pub max_buffer_bytes: usize,
    /// Maximum single NAL size emitted by the video ES assembler.
    pub max_nal_size_bytes: usize,
}

impl MpegPsConfig {
    /// Create a new config for H.264 video.
    pub fn h264() -> Self {
        Self {
            video_codec: CodecId::H264,
            max_packet_size_bytes: crate::DEFAULT_MAX_PES_SIZE,
            max_buffer_bytes: DEFAULT_MAX_BUFFER_SIZE,
            max_nal_size_bytes: DEFAULT_MAX_NAL_SIZE,
        }
    }

    /// Create a new config for H.265 video.
    pub fn h265() -> Self {
        Self {
            video_codec: CodecId::H265,
            max_packet_size_bytes: crate::DEFAULT_MAX_PES_SIZE,
            max_buffer_bytes: DEFAULT_MAX_BUFFER_SIZE,
            max_nal_size_bytes: DEFAULT_MAX_NAL_SIZE,
        }
    }
}

impl Default for MpegPsConfig {
    fn default() -> Self {
        Self::h264()
    }
}

/// Event emitted by `MpegPsDemuxer`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MpegPsEvent {
    /// A media track was discovered or its configuration changed.
    Track(TrackInfo),
    /// A compressed media packet.
    Packet(MediaPacket<'static>),
    /// End of stream.
    Eof,
}

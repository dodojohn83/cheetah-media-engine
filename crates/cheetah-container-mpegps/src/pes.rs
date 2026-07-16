//! MPEG-2 Program Stream PES packet handling.

pub use cheetah_container_mpegts::pes::PesHeader;
use cheetah_container_mpegts::pes::{
    is_audio_stream as mpegts_is_audio_stream, is_video_stream as mpegts_is_video_stream,
    parse_pes_header as mpegts_parse_pes_header,
};

use crate::MpegPsError;

/// Parse a PES header from `data` starting with the `0x000001` prefix.
pub fn parse_pes_header(data: &[u8]) -> Result<PesHeader, MpegPsError> {
    mpegts_parse_pes_header(data).map_err(map_pes_error)
}

/// True if `stream_id` indicates a video stream.
pub const fn is_video_stream(stream_id: u8) -> bool {
    mpegts_is_video_stream(stream_id)
}

/// True if `stream_id` indicates an audio stream (including private stream 1).
pub const fn is_audio_stream(stream_id: u8) -> bool {
    mpegts_is_audio_stream(stream_id)
}

fn map_pes_error(err: cheetah_container_mpegts::TsError) -> MpegPsError {
    use cheetah_container_mpegts::TsError;
    match err {
        TsError::NeedMoreData | TsError::PacketTooShort => MpegPsError::NeedMoreData,
        TsError::LostSync | TsError::InvalidInput { .. } | TsError::Unsupported { .. } => {
            MpegPsError::InvalidInput
        }
        TsError::LimitExceeded { .. } => MpegPsError::PacketTooLarge {
            size: 0,
            max: crate::DEFAULT_MAX_PES_SIZE,
        },
    }
}

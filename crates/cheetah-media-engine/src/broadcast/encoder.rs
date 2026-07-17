//! Encoder abstraction for the broadcast pipeline.
//!
//! Real platform encoders (H.264/H.265/Opus/AAC/G.711) will be implemented in
//! WP-72. The host-side placeholder returns `MediaError::Unsupported`.

use cheetah_media_types::{CodecId, MediaError, MediaPacket};

use crate::broadcast::frame::MediaFrame;

/// Capability reported by an encoder probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncoderCapability {
    /// Supported codec.
    pub codec: CodecId,
    /// Maximum coded width.
    pub max_width: u32,
    /// Maximum coded height.
    pub max_height: u32,
    /// Maximum frame rate.
    pub max_fps: u32,
    /// Bit depth (8 or 10).
    pub bit_depth: u8,
    /// Higher values are preferred when multiple encoders support a codec.
    pub priority: i32,
}

/// Encoder that converts raw `MediaFrame` values into compressed packets.
pub trait Encoder: Send {
    /// Configure the encoder for a specific codec and resolution.
    fn configure(
        &mut self,
        codec: CodecId,
        width: u32,
        height: u32,
        fps: u32,
    ) -> Result<(), MediaError>;

    /// Encode one frame. Returns a compressed packet.
    fn encode(&mut self, frame: &MediaFrame<'static>) -> Result<MediaPacket<'static>, MediaError>;

    /// Request the next output frame to be a keyframe / IDR.
    fn request_keyframe(&mut self) -> Result<(), MediaError>;

    /// Update the target bitrate in bits per second.
    fn set_bitrate(&mut self, bps: u32) -> Result<(), MediaError>;

    /// True if this encoder supports `codec` at its current configuration.
    fn supports(&self, codec: CodecId) -> bool;

    /// Human-readable encoder kind.
    fn kind(&self) -> &'static str;
}

/// Placeholder encoder used when no platform encoder is linked.
pub struct UnsupportedEncoder;

impl Encoder for UnsupportedEncoder {
    fn configure(
        &mut self,
        _codec: CodecId,
        _width: u32,
        _height: u32,
        _fps: u32,
    ) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7002,
            context: Some("encoder not linked"),
        })
    }

    fn encode(&mut self, _frame: &MediaFrame<'static>) -> Result<MediaPacket<'static>, MediaError> {
        Err(MediaError::Unsupported {
            code: 7002,
            context: Some("encoder not linked"),
        })
    }

    fn request_keyframe(&mut self) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7002,
            context: Some("encoder not linked"),
        })
    }

    fn set_bitrate(&mut self, _bps: u32) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7002,
            context: Some("encoder not linked"),
        })
    }

    fn supports(&self, _codec: CodecId) -> bool {
        false
    }

    fn kind(&self) -> &'static str {
        "unsupported"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_encoder_rejects_all_operations() {
        let mut enc = UnsupportedEncoder;
        assert!(!enc.supports(CodecId::H264));
        assert!(enc.configure(CodecId::H264, 1920, 1080, 30).is_err());
        assert!(enc.request_keyframe().is_err());
        assert!(enc.set_bitrate(1_000_000).is_err());
        assert_eq!(enc.kind(), "unsupported");
    }
}

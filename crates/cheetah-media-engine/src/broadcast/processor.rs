//! Frame processor abstraction for the broadcast pipeline.
//!
//! Processors transform `MediaFrame` values between capture and encode. Real
//! implementations (scaling, format conversion, watermarking) will be added in
//! later WPs.

use cheetah_media_types::MediaError;

use crate::broadcast::frame::MediaFrame;

/// A processor that transforms a media frame before encoding.
pub trait Processor: Send {
    /// Process `frame` and return a (possibly modified) frame.
    fn process(&mut self, frame: &MediaFrame<'static>) -> Result<MediaFrame<'static>, MediaError>;

    /// Human-readable processor kind.
    fn kind(&self) -> &'static str;
}

/// A processor that returns an owned copy of the input frame unchanged.
pub struct PassThroughProcessor;

impl Processor for PassThroughProcessor {
    fn process(&mut self, frame: &MediaFrame<'static>) -> Result<MediaFrame<'static>, MediaError> {
        Ok(frame.to_static())
    }

    fn kind(&self) -> &'static str {
        "passthrough"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broadcast::frame::MediaFrame;
    use cheetah_media_types::{MediaTime, TimeBase, Timestamp, VideoFormat, VideoFrame};

    fn sample_frame() -> MediaFrame<'static> {
        let format = VideoFormat {
            pixel_format: cheetah_media_types::PixelFormat::Rgba,
            coded_width: 2,
            coded_height: 2,
            visible_width: 2,
            visible_height: 2,
            stride: 8,
            color_space: cheetah_media_types::ColorSpace::Bt709,
        };
        let ts = Timestamp::new(0);
        MediaFrame::Video(VideoFrame::new(
            vec![0u8; 16],
            format,
            MediaTime::from_pts_dts(ts, ts, TimeBase::DEFAULT),
        ))
    }

    #[test]
    fn passthrough_returns_static_copy() {
        let mut proc = PassThroughProcessor;
        let frame = sample_frame();
        let out = proc.process(&frame).unwrap();
        assert_eq!(out, frame);
        assert_eq!(proc.kind(), "passthrough");
    }
}

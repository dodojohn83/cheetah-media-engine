//! Core media pipeline planner and scheduler.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

use alloc::boxed::Box;
use cheetah_media_abi::{AbiError, Decoder, Input, Output, Renderer};

pub mod planner;

/// A simple pipeline that pairs a decoder with an optional renderer.
#[derive(Default)]
pub struct Pipeline {
    decoder: Option<Box<dyn Decoder>>,
    renderer: Option<Box<dyn Renderer>>,
}

impl Pipeline {
    /// Create an empty pipeline.
    pub const fn new() -> Self {
        Self {
            decoder: None,
            renderer: None,
        }
    }

    /// Attach a decoder.
    pub fn set_decoder(&mut self, decoder: Box<dyn Decoder>) {
        self.decoder = Some(decoder);
    }

    /// Attach a renderer.
    pub fn set_renderer(&mut self, renderer: Box<dyn Renderer>) {
        self.renderer = Some(renderer);
    }

    /// Feed a compressed sample through the decoder.
    pub fn feed(&mut self, input: &Input<'_>) -> Result<Output, AbiError> {
        let decoder = self.decoder.as_mut().ok_or(AbiError::NotSupported)?;
        let output = decoder.decode(input)?;
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.render(&output)?;
        }
        Ok(output)
    }

    /// Flush the pipeline.
    pub fn flush(&mut self) -> Result<(), AbiError> {
        if let Some(decoder) = self.decoder.as_mut() {
            decoder.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_abi::{AbiError, DecoderProbe, Input, Output, Renderer};
    use cheetah_media_types::{CodecId, MediaTime, TimeBase, Timestamp, TrackId};

    struct DummyDecoder;
    impl DecoderProbe for DummyDecoder {
        fn supports(&self, _codec: CodecId) -> bool {
            true
        }
    }
    impl Decoder for DummyDecoder {
        fn decode(&mut self, input: &Input<'_>) -> Result<Output, AbiError> {
            Ok(Output {
                data: input.data.to_vec(),
                time: input.time,
                duration_ms: 0,
                track_id: input.track_id,
            })
        }
        fn flush(&mut self) -> Result<(), AbiError> {
            Ok(())
        }
    }

    struct DummyRenderer;
    impl Renderer for DummyRenderer {
        fn render(&mut self, _output: &Output) -> Result<(), AbiError> {
            Ok(())
        }
        fn set_viewport(&mut self, _width: u32, _height: u32) -> Result<(), AbiError> {
            Ok(())
        }
    }

    fn dummy_time() -> MediaTime {
        MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT)
    }

    fn dummy_track() -> TrackId {
        TrackId::new(1).unwrap()
    }

    #[test]
    fn feed_without_decoder_fails() {
        let mut pipeline = Pipeline::new();
        let input = Input {
            data: &[],
            time: dummy_time(),
            codec: CodecId::H264,
            track_id: dummy_track(),
        };
        assert_eq!(pipeline.feed(&input).unwrap_err(), AbiError::NotSupported);
    }

    #[test]
    fn feed_with_decoder_and_renderer_passes() {
        let mut pipeline = Pipeline::new();
        pipeline.set_decoder(Box::new(DummyDecoder));
        pipeline.set_renderer(Box::new(DummyRenderer));
        let input = Input {
            data: b"data",
            time: dummy_time(),
            codec: CodecId::H264,
            track_id: dummy_track(),
        };
        assert!(pipeline.feed(&input).is_ok());
    }
}

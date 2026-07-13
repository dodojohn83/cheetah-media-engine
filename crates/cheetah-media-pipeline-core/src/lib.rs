//! Core media pipeline planner and scheduler.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

use alloc::boxed::Box;
use cheetah_media_abi::{Decoder, Error, Input, Output, Renderer};

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
    pub fn feed<'a>(&mut self, input: &Input<'a>) -> Result<Output<'a>, Error> {
        let decoder = self.decoder.as_mut().ok_or(Error::NotSupported)?;
        let output = decoder.decode(input)?;
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.render(&output)?;
        }
        Ok(output)
    }

    /// Flush the pipeline.
    pub fn flush(&mut self) -> Result<(), Error> {
        if let Some(decoder) = self.decoder.as_mut() {
            decoder.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_abi::{DecoderProbe, Error, Input, Output, Renderer};
    use cheetah_media_types::{CodecId, MediaTime};

    struct DummyDecoder;
    impl DecoderProbe for DummyDecoder {
        fn supports(&self, _codec: CodecId) -> bool {
            true
        }
    }
    impl Decoder for DummyDecoder {
        fn decode<'a>(&mut self, input: &Input<'a>) -> Result<Output<'a>, Error> {
            Ok(Output {
                data: input.data,
                pts: input.pts,
                duration_ms: 0,
            })
        }
        fn flush(&mut self) -> Result<(), Error> {
            Ok(())
        }
    }

    struct DummyRenderer;
    impl Renderer for DummyRenderer {
        fn render(&mut self, _output: &Output) -> Result<(), Error> {
            Ok(())
        }
        fn set_viewport(&mut self, _width: u32, _height: u32) -> Result<(), Error> {
            Ok(())
        }
    }

    #[test]
    fn feed_without_decoder_fails() {
        let mut pipeline = Pipeline::new();
        let input = Input {
            data: &[],
            pts: MediaTime::default(),
            dts: MediaTime::default(),
            codec: CodecId::H264,
        };
        assert_eq!(pipeline.feed(&input).unwrap_err(), Error::NotSupported);
    }

    #[test]
    fn feed_with_decoder_and_renderer_passes() {
        let mut pipeline = Pipeline::new();
        pipeline.set_decoder(Box::new(DummyDecoder));
        pipeline.set_renderer(Box::new(DummyRenderer));
        let input = Input {
            data: b"data",
            pts: MediaTime::default(),
            dts: MediaTime::default(),
            codec: CodecId::H264,
        };
        let output = pipeline.feed(&input).unwrap();
        assert_eq!(output.data, b"data");
    }
}

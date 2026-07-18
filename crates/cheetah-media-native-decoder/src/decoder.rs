//! `NativeDecoder` with hardware-first fallback selection.

use alloc::boxed::Box;
use alloc::vec::Vec;

use cheetah_media_abi::{AbiError, Decoder, DecoderProbe, Input, Output};
use cheetah_media_bitstream::g711::G711Kind;
use cheetah_media_types::CodecId;

use crate::capability::PlatformApi;
use crate::g711::G711Decoder;
use crate::registry::CapabilityRegistry;

/// A decoder that tries a list of backends in order, falling back when a
/// backend reports `AbiError::NotSupported`.
pub struct NativeDecoder {
    backends: Vec<Box<dyn Decoder + Send>>,
}

impl NativeDecoder {
    /// Create a decoder from a registry and codec.
    ///
    /// For video codecs, provide `(width, height, fps)`. For audio, pass `None`.
    pub fn from_registry(
        registry: &CapabilityRegistry,
        codec: CodecId,
        video: Option<(u32, u32, u32)>,
    ) -> Result<Self, AbiError> {
        let mut backends: Vec<Box<dyn Decoder + Send>> = Vec::new();

        let selected = match video {
            Some((width, height, fps)) => registry.select(codec, width, height, fps),
            None => registry.select_audio(codec),
        };

        if let Some(PlatformApi::Software) = selected {
            match codec {
                CodecId::G711A => backends.push(Box::new(G711Decoder::new(G711Kind::ALaw))),
                CodecId::G711U => backends.push(Box::new(G711Decoder::new(G711Kind::MuLaw))),
                _ => {}
            }
        }

        if backends.is_empty() {
            return Err(AbiError::NotSupported);
        }

        Ok(Self { backends })
    }

    /// Create a decoder with an explicit backend list. Useful for tests and
    /// for overriding registry selection.
    pub fn with_backends(backends: Vec<Box<dyn Decoder + Send>>) -> Self {
        Self { backends }
    }
}

impl DecoderProbe for NativeDecoder {
    fn supports(&self, codec: CodecId) -> bool {
        self.backends.iter().any(|b| b.supports(codec))
    }
}

impl Decoder for NativeDecoder {
    fn decode(&mut self, input: &Input<'_>) -> Result<Output, AbiError> {
        if self.backends.is_empty() {
            return Err(AbiError::NotSupported);
        }

        let mut last_error = AbiError::NotSupported;
        for backend in self.backends.iter_mut() {
            if !backend.supports(input.codec) {
                continue;
            }
            match backend.decode(input) {
                Ok(output) => return Ok(output),
                Err(AbiError::NotSupported) | Err(AbiError::WouldBlock) => {
                    last_error = AbiError::NotSupported;
                    continue;
                }
                Err(e) => {
                    last_error = e;
                    continue;
                }
            }
        }
        Err(last_error)
    }

    fn flush(&mut self) -> Result<(), AbiError> {
        for backend in self.backends.iter_mut() {
            backend.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_abi::{DecoderProbe, Input, Output};
    use cheetah_media_types::{CodecId, MediaTime, TimeBase, Timestamp, TrackId};

    fn input(data: &[u8], codec: CodecId) -> Input<'_> {
        Input {
            data,
            time: MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT),
            codec,
            track_id: TrackId::new(1).unwrap(),
        }
    }

    struct FailingHardwareDecoder;
    impl DecoderProbe for FailingHardwareDecoder {
        fn supports(&self, codec: CodecId) -> bool {
            codec == CodecId::H264
        }
    }
    impl Decoder for FailingHardwareDecoder {
        fn decode(&mut self, _input: &Input<'_>) -> Result<Output, AbiError> {
            Err(AbiError::NotSupported)
        }
        fn flush(&mut self) -> Result<(), AbiError> {
            Ok(())
        }
    }

    struct PassingSoftwareDecoder;
    impl DecoderProbe for PassingSoftwareDecoder {
        fn supports(&self, codec: CodecId) -> bool {
            codec == CodecId::H264
        }
    }
    impl Decoder for PassingSoftwareDecoder {
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

    #[test]
    fn fallback_to_software_after_hardware_not_supported() {
        let mut dec = NativeDecoder::with_backends(vec![
            Box::new(FailingHardwareDecoder),
            Box::new(PassingSoftwareDecoder),
        ]);
        let out = dec.decode(&input(b"frame", CodecId::H264)).unwrap();
        assert_eq!(out.data, b"frame");
    }

    #[test]
    fn no_backend_returns_not_supported() {
        let mut dec = NativeDecoder::with_backends(vec![]);
        assert_eq!(
            dec.decode(&input(b"x", CodecId::H265)).unwrap_err(),
            AbiError::NotSupported
        );
    }

    #[test]
    fn g711_from_registry_works() {
        let mut reg = CapabilityRegistry::new();
        reg.register(crate::probe::SoftwareProbe);
        let mut dec = NativeDecoder::from_registry(&reg, CodecId::G711A, None).unwrap();
        assert!(dec.supports(CodecId::G711A));
        let out = dec.decode(&input(&[0x55, 0x55], CodecId::G711A)).unwrap();
        assert_eq!(out.data.len(), 4);
    }

    #[test]
    fn from_registry_rejects_unsupported_codec() {
        let mut reg = CapabilityRegistry::new();
        reg.register(crate::probe::SoftwareProbe);
        let result = NativeDecoder::from_registry(&reg, CodecId::H264, Some((1920, 1080, 30)));
        assert!(matches!(result, Err(AbiError::NotSupported)));
    }
}

//! Software G.711 A-law / μ-law decoder.

use alloc::vec::Vec;

use cheetah_media_abi::{AbiError, Decoder, DecoderProbe, Input, Output};
pub use cheetah_media_bitstream::g711::{self, G711Kind};
use cheetah_media_types::CodecId;

/// Decodes G.711 A-law or μ-law into 16-bit signed PCM.
pub struct G711Decoder {
    kind: G711Kind,
    output: Vec<u8>,
}

impl G711Decoder {
    /// Create a decoder for the given G.711 variant.
    pub fn new(kind: G711Kind) -> Self {
        Self {
            kind,
            output: Vec::with_capacity(1024),
        }
    }
}

impl DecoderProbe for G711Decoder {
    fn supports(&self, codec: CodecId) -> bool {
        matches!(
            (codec, self.kind),
            (CodecId::G711A, G711Kind::ALaw) | (CodecId::G711U, G711Kind::MuLaw)
        )
    }
}

impl Decoder for G711Decoder {
    fn decode(&mut self, input: &Input<'_>) -> Result<Output, AbiError> {
        if !self.supports(input.codec) {
            return Err(AbiError::NotSupported);
        }

        let samples = input.data.len();
        if samples > usize::MAX / 2 {
            return Err(AbiError::OutOfBounds);
        }
        self.output.clear();
        self.output.reserve(samples * 2);

        for &sample in input.data {
            let pcm = g711::decode(self.kind, sample);
            self.output.extend_from_slice(&pcm.to_le_bytes());
        }

        Ok(Output {
            data: core::mem::take(&mut self.output),
            time: input.time,
            duration_ms: 0,
            track_id: input.track_id,
        })
    }

    fn flush(&mut self) -> Result<(), AbiError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_types::{MediaTime, TimeBase, Timestamp, TrackId};

    fn input(data: &[u8], codec: CodecId) -> Input<'_> {
        Input {
            data,
            time: MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT),
            codec,
            track_id: TrackId::new(1).unwrap(),
        }
    }

    #[test]
    fn alaw_roundtrip() {
        // 0x55 is A-law silence-ish sample; decode should produce a finite PCM value.
        let mut dec = G711Decoder::new(G711Kind::ALaw);
        let out = dec.decode(&input(&[0x55, 0x55], CodecId::G711A)).unwrap();
        assert_eq!(out.data.len(), 4);
        assert!(dec.supports(CodecId::G711A));
        assert!(!dec.supports(CodecId::G711U));
    }

    #[test]
    fn ulaw_roundtrip() {
        let mut dec = G711Decoder::new(G711Kind::MuLaw);
        let out = dec.decode(&input(&[0xff, 0x7f], CodecId::G711U)).unwrap();
        assert_eq!(out.data.len(), 4);
        assert!(dec.supports(CodecId::G711U));
        assert!(!dec.supports(CodecId::G711A));
    }

    #[test]
    fn unsupported_codec_rejected() {
        let mut dec = G711Decoder::new(G711Kind::ALaw);
        let err = dec.decode(&input(&[0x55], CodecId::H264)).unwrap_err();
        assert_eq!(err, AbiError::NotSupported);
    }
}

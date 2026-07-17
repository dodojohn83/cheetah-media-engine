//! Android `MediaCodec` decoder implementation.
//!
//! The host-side stub reports `AbiError::NotSupported` for every decode call.
//! The real JNI-backed `MediaCodec` path is implemented once the Android NDK
//! is linked and the `target_os = "android"` path can be compiled.

use cheetah_media_abi::{AbiError, Decoder, DecoderProbe, Input, Output};
use cheetah_media_types::CodecId;

/// Decoder backed by Android `MediaCodec`.
pub struct AndroidDecoder;

impl AndroidDecoder {
    /// Create a new Android decoder instance.
    pub const fn new() -> Self {
        Self
    }
}

impl Default for AndroidDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl DecoderProbe for AndroidDecoder {
    fn supports(&self, _codec: CodecId) -> bool {
        // The host stub cannot decode anything. The Android build will query
        // MediaCodecInfo for supported codecs.
        false
    }
}

impl Decoder for AndroidDecoder {
    fn decode(&mut self, _input: &Input<'_>) -> Result<Output, AbiError> {
        Err(AbiError::NotSupported)
    }

    fn flush(&mut self) -> Result<(), AbiError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_abi::{DecoderProbe, Input};
    use cheetah_media_types::{CodecId, MediaTime, TimeBase, Timestamp, TrackId};

    fn make_input(data: &[u8], codec: CodecId) -> Input<'_> {
        Input {
            data,
            time: MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT),
            codec,
            track_id: TrackId::new(1).unwrap(),
        }
    }

    #[test]
    fn host_stub_rejects_all_codecs() {
        let mut dec = AndroidDecoder::new();
        assert!(!dec.supports(CodecId::H264));
        assert_eq!(
            dec.decode(&make_input(b"frame", CodecId::H264))
                .unwrap_err(),
            AbiError::NotSupported
        );
    }

    #[test]
    fn flush_is_idempotent() {
        let mut dec = AndroidDecoder::new();
        assert!(dec.flush().is_ok());
        assert!(dec.flush().is_ok());
    }
}

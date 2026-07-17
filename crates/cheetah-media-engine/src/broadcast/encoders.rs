//! Concrete encoder implementations for the broadcast pipeline.
//!
//! WP-72 adds host-side stubs for H.264/H.265/Opus/AAC and a working G.711
//! encoder. Real hardware/platform encoders will replace the stubs in later
//! work packages.

use alloc::vec::Vec;

use cheetah_media_bitstream::g711::{self, G711Kind};
use cheetah_media_types::{
    ChannelLayout, CodecId, MediaError, MediaPacket, MediaTime, SampleFormat, SequenceNumber,
    StreamEpoch, TrackId,
};

use crate::broadcast::encoder::{Encoder, EncoderCapability};
use crate::broadcast::frame::MediaFrame;

/// Shared capability table for host-side stubs: no capabilities.
const EMPTY_CAPABILITIES: &[EncoderCapability] = &[];

/// Placeholder H.264 video encoder.
pub struct H264Encoder;

impl Encoder for H264Encoder {
    fn configure(
        &mut self,
        _codec: CodecId,
        _width: u32,
        _height: u32,
        _fps: u32,
    ) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7201,
            context: Some("H.264 encoder not linked"),
        })
    }

    fn encode(
        &mut self,
        _frame: &MediaFrame<'static>,
        _track_id: TrackId,
        _stream_epoch: StreamEpoch,
        _sequence: SequenceNumber,
    ) -> Result<MediaPacket<'static>, MediaError> {
        Err(MediaError::Unsupported {
            code: 7201,
            context: Some("H.264 encoder not linked"),
        })
    }

    fn request_keyframe(&mut self) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7201,
            context: Some("H.264 encoder not linked"),
        })
    }

    fn set_bitrate(&mut self, _bps: u32) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7201,
            context: Some("H.264 encoder not linked"),
        })
    }

    fn capabilities(&self) -> &[EncoderCapability] {
        EMPTY_CAPABILITIES
    }

    fn kind(&self) -> &'static str {
        "h264"
    }
}

/// Placeholder H.265 video encoder.
pub struct H265Encoder;

impl Encoder for H265Encoder {
    fn configure(
        &mut self,
        _codec: CodecId,
        _width: u32,
        _height: u32,
        _fps: u32,
    ) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7202,
            context: Some("H.265 encoder not linked"),
        })
    }

    fn encode(
        &mut self,
        _frame: &MediaFrame<'static>,
        _track_id: TrackId,
        _stream_epoch: StreamEpoch,
        _sequence: SequenceNumber,
    ) -> Result<MediaPacket<'static>, MediaError> {
        Err(MediaError::Unsupported {
            code: 7202,
            context: Some("H.265 encoder not linked"),
        })
    }

    fn request_keyframe(&mut self) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7202,
            context: Some("H.265 encoder not linked"),
        })
    }

    fn set_bitrate(&mut self, _bps: u32) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7202,
            context: Some("H.265 encoder not linked"),
        })
    }

    fn capabilities(&self) -> &[EncoderCapability] {
        EMPTY_CAPABILITIES
    }

    fn kind(&self) -> &'static str {
        "h265"
    }
}

/// Placeholder Opus audio encoder.
pub struct OpusEncoder;

impl Encoder for OpusEncoder {
    fn configure(
        &mut self,
        _codec: CodecId,
        _width: u32,
        _height: u32,
        _fps: u32,
    ) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7203,
            context: Some("Opus encoder not linked"),
        })
    }

    fn encode(
        &mut self,
        _frame: &MediaFrame<'static>,
        _track_id: TrackId,
        _stream_epoch: StreamEpoch,
        _sequence: SequenceNumber,
    ) -> Result<MediaPacket<'static>, MediaError> {
        Err(MediaError::Unsupported {
            code: 7203,
            context: Some("Opus encoder not linked"),
        })
    }

    fn request_keyframe(&mut self) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7203,
            context: Some("Opus encoder not linked"),
        })
    }

    fn set_bitrate(&mut self, _bps: u32) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7203,
            context: Some("Opus encoder not linked"),
        })
    }

    fn capabilities(&self) -> &[EncoderCapability] {
        EMPTY_CAPABILITIES
    }

    fn kind(&self) -> &'static str {
        "opus"
    }
}

/// Placeholder AAC audio encoder.
pub struct AacEncoder;

impl Encoder for AacEncoder {
    fn configure(
        &mut self,
        _codec: CodecId,
        _width: u32,
        _height: u32,
        _fps: u32,
    ) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7204,
            context: Some("AAC encoder not linked"),
        })
    }

    fn encode(
        &mut self,
        _frame: &MediaFrame<'static>,
        _track_id: TrackId,
        _stream_epoch: StreamEpoch,
        _sequence: SequenceNumber,
    ) -> Result<MediaPacket<'static>, MediaError> {
        Err(MediaError::Unsupported {
            code: 7204,
            context: Some("AAC encoder not linked"),
        })
    }

    fn request_keyframe(&mut self) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7204,
            context: Some("AAC encoder not linked"),
        })
    }

    fn set_bitrate(&mut self, _bps: u32) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7204,
            context: Some("AAC encoder not linked"),
        })
    }

    fn capabilities(&self) -> &[EncoderCapability] {
        EMPTY_CAPABILITIES
    }

    fn kind(&self) -> &'static str {
        "aac"
    }
}

/// G.711 A-law / μ-law audio encoder.
///
/// This is a pure-Rust encoder using `cheetah-media-bitstream::g711`. It accepts
/// interleaved S16 or F32 PCM and produces one compressed byte per sample.
pub struct G711Encoder {
    kind: G711Kind,
}

impl G711Encoder {
    /// Create a G.711 encoder configured for A-law or μ-law.
    pub const fn new(kind: G711Kind) -> Self {
        Self { kind }
    }

    fn encode_s16(&self, input: &[u8]) -> Vec<u8> {
        let sample_count = input.len() / 2;
        let mut output = Vec::with_capacity(sample_count);
        for chunk in input.chunks_exact(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            output.push(g711::encode(self.kind, sample));
        }
        output
    }

    fn encode_f32(&self, input: &[u8]) -> Vec<u8> {
        let sample_count = input.len() / 4;
        let mut output = Vec::with_capacity(sample_count);
        for chunk in input.chunks_exact(4) {
            let bits = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let sample = f32::from_bits(bits);
            output.push(g711::encode_f32(self.kind, sample));
        }
        output
    }

    fn build_packet(
        &self,
        payload: Vec<u8>,
        time: MediaTime,
        track_id: TrackId,
        stream_epoch: StreamEpoch,
        sequence: SequenceNumber,
    ) -> MediaPacket<'static> {
        MediaPacket::new(payload, track_id, stream_epoch, sequence, time).with_keyframe()
    }
}

impl Encoder for G711Encoder {
    fn configure(
        &mut self,
        codec: CodecId,
        _width: u32,
        _height: u32,
        _fps: u32,
    ) -> Result<(), MediaError> {
        match codec {
            CodecId::G711A => self.kind = G711Kind::ALaw,
            CodecId::G711U => self.kind = G711Kind::MuLaw,
            _ => {
                return Err(MediaError::InvalidInput {
                    code: 7205,
                    context: Some("G711Encoder only supports G711A/G711U"),
                });
            }
        }
        Ok(())
    }

    fn encode(
        &mut self,
        frame: &MediaFrame<'static>,
        track_id: TrackId,
        stream_epoch: StreamEpoch,
        sequence: SequenceNumber,
    ) -> Result<MediaPacket<'static>, MediaError> {
        let audio = match frame {
            MediaFrame::Audio(audio) => audio,
            MediaFrame::Video(_) => {
                return Err(MediaError::InvalidInput {
                    code: 7206,
                    context: Some("G711Encoder expects an AudioFrame"),
                });
            }
        };

        let format = audio.format;
        if format.channel_layout != ChannelLayout::Mono {
            return Err(MediaError::InvalidInput {
                code: 7211,
                context: Some("G711Encoder only supports mono audio"),
            });
        }

        let bps = format.bytes_per_sample() as usize;
        let sample_count = format.sample_count as usize;
        let expected = sample_count * bps;

        let is_planar = matches!(
            format.sample_format,
            SampleFormat::S16Planar
                | SampleFormat::S32Planar
                | SampleFormat::F32Planar
                | SampleFormat::F64Planar
        );
        let input = if is_planar {
            if audio.planes.is_empty() {
                return Err(MediaError::InvalidInput {
                    code: 7208,
                    context: Some("G711Encoder planar frame has no planes"),
                });
            }
            audio.planes[0].as_ref()
        } else {
            audio.payload.as_ref()
        };

        if input.len() < expected || input.len() % bps != 0 {
            return Err(MediaError::InvalidInput {
                code: 7209,
                context: Some("G711Encoder input length is not sample-aligned"),
            });
        }
        let input = &input[..expected];

        let payload = match format.sample_format {
            SampleFormat::S16 | SampleFormat::S16Planar => self.encode_s16(input),
            SampleFormat::F32 | SampleFormat::F32Planar => self.encode_f32(input),
            _ => {
                return Err(MediaError::InvalidInput {
                    code: 7207,
                    context: Some("G711Encoder only supports S16 or F32 PCM"),
                });
            }
        };

        Ok(self.build_packet(payload, audio.timestamp, track_id, stream_epoch, sequence))
    }

    fn request_keyframe(&mut self) -> Result<(), MediaError> {
        // Audio packets are all keyframes; this is a no-op.
        Ok(())
    }

    fn set_bitrate(&mut self, _bps: u32) -> Result<(), MediaError> {
        // G.711 is constant bitrate; ignore bitrate changes.
        Ok(())
    }

    fn capabilities(&self) -> &[EncoderCapability] {
        &G711_CAPABILITIES
    }

    fn kind(&self) -> &'static str {
        "g711"
    }
}

const G711_CAPABILITIES: [EncoderCapability; 2] = [
    EncoderCapability {
        codec: CodecId::G711A,
        max_width: 0,
        max_height: 0,
        max_fps: 0,
        bit_depth: 8,
        priority: 10,
    },
    EncoderCapability {
        codec: CodecId::G711U,
        max_width: 0,
        max_height: 0,
        max_fps: 0,
        bit_depth: 8,
        priority: 10,
    },
];

/// Mock encoder for headless pipeline tests.
pub struct MockEncoder {
    configured_codec: Option<CodecId>,
}

impl Default for MockEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockEncoder {
    /// Create a mock encoder that reports support for `CodecId::H264`.
    pub const fn new() -> Self {
        Self {
            configured_codec: None,
        }
    }
}

impl Encoder for MockEncoder {
    fn configure(
        &mut self,
        codec: CodecId,
        _width: u32,
        _height: u32,
        _fps: u32,
    ) -> Result<(), MediaError> {
        self.configured_codec = Some(codec);
        Ok(())
    }

    fn encode(
        &mut self,
        frame: &MediaFrame<'static>,
        track_id: TrackId,
        stream_epoch: StreamEpoch,
        sequence: SequenceNumber,
    ) -> Result<MediaPacket<'static>, MediaError> {
        let payload = alloc::vec![frame.timestamp().pts_ms().unwrap_or(0) as u8];
        Ok(
            MediaPacket::new(payload, track_id, stream_epoch, sequence, frame.timestamp())
                .with_keyframe(),
        )
    }

    fn request_keyframe(&mut self) -> Result<(), MediaError> {
        Ok(())
    }

    fn set_bitrate(&mut self, _bps: u32) -> Result<(), MediaError> {
        Ok(())
    }

    fn capabilities(&self) -> &[EncoderCapability] {
        &MOCK_CAPABILITIES
    }

    fn kind(&self) -> &'static str {
        "mock"
    }
}

const MOCK_CAPABILITIES: [EncoderCapability; 1] = [EncoderCapability {
    codec: CodecId::H264,
    max_width: 1920,
    max_height: 1080,
    max_fps: 60,
    bit_depth: 8,
    priority: 5,
}];

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_types::{AudioFormat, ChannelLayout, MediaTime, TimeBase, Timestamp};

    fn audio_frame(sample_format: SampleFormat, samples: Vec<u8>) -> MediaFrame<'static> {
        let format = AudioFormat {
            sample_format,
            sample_rate: 8000,
            channel_layout: ChannelLayout::Mono,
            sample_count: (samples.len() / sample_format.bytes_per_sample() as usize) as u32,
        };
        let ts = MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
        MediaFrame::Audio(cheetah_media_types::AudioFrame::new(samples, format, ts))
    }

    #[test]
    fn g711a_encoder_configures_and_encodes_s16() {
        let mut enc = G711Encoder::new(G711Kind::ALaw);
        enc.configure(CodecId::G711A, 0, 0, 0).unwrap();

        let samples = alloc::vec![0i16, 100, -100, 1000, -1000]
            .into_iter()
            .flat_map(|s: i16| s.to_le_bytes())
            .collect();
        let frame = audio_frame(SampleFormat::S16, samples);

        let packet = enc
            .encode(
                &frame,
                TrackId::new(1).unwrap(),
                StreamEpoch::new(0),
                SequenceNumber::new(0),
            )
            .unwrap();

        assert_eq!(packet.payload.len(), 5);
        assert!(packet.flags.is_keyframe);
        assert_eq!(packet.track_id.get(), 1);
    }

    #[test]
    fn g711u_encoder_configures_and_encodes_f32() {
        let mut enc = G711Encoder::new(G711Kind::MuLaw);
        enc.configure(CodecId::G711U, 0, 0, 0).unwrap();

        let samples: Vec<f32> = alloc::vec![0.0, 0.5, -0.5];
        let bytes: Vec<u8> = samples.iter().flat_map(|s: &f32| s.to_le_bytes()).collect();
        let frame = audio_frame(SampleFormat::F32, bytes);

        let packet = enc
            .encode(
                &frame,
                TrackId::new(1).unwrap(),
                StreamEpoch::new(0),
                SequenceNumber::new(0),
            )
            .unwrap();

        assert_eq!(packet.payload.len(), 3);
        assert!(packet.flags.is_keyframe);
    }

    #[test]
    fn g711_encoder_rejects_unaligned_and_short_input() {
        let mut enc = G711Encoder::new(G711Kind::ALaw);
        enc.configure(CodecId::G711A, 0, 0, 0).unwrap();

        // One trailing byte makes the buffer non-sample-aligned.
        let mut samples = alloc::vec![0i16, 100, -100]
            .into_iter()
            .flat_map(|s: i16| s.to_le_bytes())
            .collect::<Vec<u8>>();
        samples.push(0xab);
        let frame = audio_frame(SampleFormat::S16, samples);
        assert!(
            enc.encode(
                &frame,
                TrackId::new(1).unwrap(),
                StreamEpoch::new(0),
                SequenceNumber::new(0),
            )
            .is_err()
        );

        // sample_count claims five S16 samples but only four bytes are supplied.
        let samples = alloc::vec![0u8; 4];
        let format = AudioFormat {
            sample_format: SampleFormat::S16,
            sample_rate: 8000,
            channel_layout: ChannelLayout::Mono,
            sample_count: 5,
        };
        let ts = MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
        let frame = MediaFrame::Audio(cheetah_media_types::AudioFrame::new(samples, format, ts));
        assert!(
            enc.encode(
                &frame,
                TrackId::new(1).unwrap(),
                StreamEpoch::new(0),
                SequenceNumber::new(0),
            )
            .is_err()
        );
    }

    #[test]
    fn g711_encoder_rejects_non_mono_and_encodes_planar_mono() {
        let mut enc = G711Encoder::new(G711Kind::ALaw);
        enc.configure(CodecId::G711A, 0, 0, 0).unwrap();

        let samples = alloc::vec![0i16, 100, -100]
            .into_iter()
            .flat_map(|s: i16| s.to_le_bytes())
            .collect::<Vec<u8>>();
        let mut format = AudioFormat {
            sample_format: SampleFormat::S16,
            sample_rate: 8000,
            channel_layout: ChannelLayout::Stereo,
            sample_count: 3,
        };
        let ts = MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
        let frame = MediaFrame::Audio(cheetah_media_types::AudioFrame::new(
            samples.clone(),
            format,
            ts,
        ));
        assert!(
            enc.encode(
                &frame,
                TrackId::new(1).unwrap(),
                StreamEpoch::new(0),
                SequenceNumber::new(0),
            )
            .is_err()
        );

        // Planar mono: payload is empty, single plane carries the samples.
        format.channel_layout = ChannelLayout::Mono;
        format.sample_format = SampleFormat::S16Planar;
        let frame = MediaFrame::Audio(
            cheetah_media_types::AudioFrame::new(Vec::new(), format, ts).with_plane(samples),
        );
        let packet = enc
            .encode(
                &frame,
                TrackId::new(1).unwrap(),
                StreamEpoch::new(0),
                SequenceNumber::new(0),
            )
            .unwrap();
        assert_eq!(packet.payload.len(), 3);
    }

    #[test]
    fn g711_encoder_rejects_video_frame() {
        let mut enc = G711Encoder::new(G711Kind::ALaw);
        enc.configure(CodecId::G711A, 0, 0, 0).unwrap();

        let fmt = cheetah_media_types::VideoFormat {
            pixel_format: cheetah_media_types::PixelFormat::Rgba,
            coded_width: 2,
            coded_height: 2,
            visible_width: 2,
            visible_height: 2,
            stride: 8,
            color_space: cheetah_media_types::ColorSpace::Bt709,
        };
        let ts = MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
        let frame = MediaFrame::Video(cheetah_media_types::VideoFrame::new(
            alloc::vec![0u8; 16],
            fmt,
            ts,
        ));

        assert!(
            enc.encode(
                &frame,
                TrackId::new(1).unwrap(),
                StreamEpoch::new(0),
                SequenceNumber::new(0),
            )
            .is_err()
        );
    }

    #[test]
    fn host_stubs_reject_configuration() {
        assert!(
            H264Encoder
                .configure(CodecId::H264, 1920, 1080, 30)
                .is_err()
        );
        assert!(
            H265Encoder
                .configure(CodecId::H265, 1920, 1080, 30)
                .is_err()
        );
        assert!(OpusEncoder.configure(CodecId::Opus, 0, 0, 0).is_err());
        assert!(AacEncoder.configure(CodecId::Aac, 0, 0, 0).is_err());
    }

    #[test]
    fn mock_encoder_produces_packet() {
        let mut enc = MockEncoder::new();
        enc.configure(CodecId::H264, 64, 64, 30).unwrap();

        let fmt = cheetah_media_types::VideoFormat {
            pixel_format: cheetah_media_types::PixelFormat::Rgba,
            coded_width: 2,
            coded_height: 2,
            visible_width: 2,
            visible_height: 2,
            stride: 8,
            color_space: cheetah_media_types::ColorSpace::Bt709,
        };
        let ts = MediaTime::from_pts_dts(Timestamp::new(42), Timestamp::new(42), TimeBase::DEFAULT);
        let frame = MediaFrame::Video(cheetah_media_types::VideoFrame::new(
            alloc::vec![0u8; 16],
            fmt,
            ts,
        ));

        let packet = enc
            .encode(
                &frame,
                TrackId::new(7).unwrap(),
                StreamEpoch::new(2),
                SequenceNumber::new(99),
            )
            .unwrap();

        assert_eq!(packet.track_id.get(), 7);
        assert_eq!(packet.stream_epoch.get(), 2);
        assert_eq!(packet.sequence.get(), 99);
        assert!(packet.flags.is_keyframe);
    }
}

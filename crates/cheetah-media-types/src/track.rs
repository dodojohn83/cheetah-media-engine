//! Track identifiers and track descriptors.

use crate::{AudioFormat, CodecId, MediaError, TimeBase, TrackKind, VideoFormat};

/// Identifies a track within a presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrackId(u32);

impl TrackId {
    /// Track 0 is reserved; valid IDs start at 1.
    pub const fn new(id: u32) -> Option<Self> {
        if id == 0 { None } else { Some(Self(id)) }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Monotonically increasing stream generation/epoch used to fence stale data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamEpoch(u64);

impl StreamEpoch {
    pub const fn new(epoch: u64) -> Self {
        Self(epoch)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// A sequence number for ordering within a track or transport session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SequenceNumber(u64);

impl SequenceNumber {
    pub const fn new(seq: u64) -> Self {
        Self(seq)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// Codec-specific configuration bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum CodecConfig {
    /// No configuration available.
    #[default]
    None,
    /// H.264 / AVC codec configuration in AVCC form.
    AvcC(alloc::vec::Vec<u8>),
    /// H.265 / HEVC codec configuration.
    HevcC(alloc::vec::Vec<u8>),
    /// AAC AudioSpecificConfig.
    AacAudioSpecificConfig(alloc::vec::Vec<u8>),
    /// Opus header.
    OpusHeader(alloc::vec::Vec<u8>),
    /// Raw bytes with an identifying four-character code.
    Raw(&'static str, alloc::vec::Vec<u8>),
}

impl CodecConfig {
    /// True if configuration bytes are present.
    pub fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// Return the raw bytes, if any.
    pub fn bytes(&self) -> Option<&[u8]> {
        match self {
            Self::None => None,
            Self::AvcC(v)
            | Self::HevcC(v)
            | Self::AacAudioSpecificConfig(v)
            | Self::OpusHeader(v)
            | Self::Raw(_, v) => Some(v),
        }
    }

    /// Increment the generation if this config replaces a different one.
    pub fn generation_delta(&self, other: &Self) -> u64 {
        if self == other { 0 } else { 1 }
    }
}

/// Static description of a media track.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrackInfo {
    pub id: TrackId,
    pub kind: TrackKind,
    pub codec: CodecId,
    pub timebase: TimeBase,
    pub codec_config: CodecConfig,
    pub video_format: Option<VideoFormat>,
    pub audio_format: Option<AudioFormat>,
    pub generation: u64,
}

impl TrackInfo {
    /// Create a new `TrackInfo` with generation 0.
    pub fn new(id: TrackId, kind: TrackKind, codec: CodecId, timebase: TimeBase) -> Self {
        Self {
            id,
            kind,
            codec,
            timebase,
            codec_config: CodecConfig::None,
            video_format: None,
            audio_format: None,
            generation: 0,
        }
    }

    /// Update the codec configuration and increment `generation` if it changed.
    pub fn set_codec_config(&mut self, config: CodecConfig) {
        if self.codec_config != config {
            self.generation += 1;
        }
        self.codec_config = config;
    }

    /// Update the video format and increment `generation` if it changed.
    pub fn set_video_format(&mut self, format: VideoFormat) -> Result<(), MediaError> {
        if self.kind != TrackKind::Video {
            return Err(MediaError::InvalidInput {
                code: 1002,
                context: Some("video_format on non-video track"),
            });
        }
        if self.video_format.as_ref() != Some(&format) {
            self.generation += 1;
        }
        self.video_format = Some(format);
        Ok(())
    }

    /// Update the audio format and increment `generation` if it changed.
    pub fn set_audio_format(&mut self, format: AudioFormat) -> Result<(), MediaError> {
        if self.kind != TrackKind::Audio {
            return Err(MediaError::InvalidInput {
                code: 1003,
                context: Some("audio_format on non-audio track"),
            });
        }
        if self.audio_format.as_ref() != Some(&format) {
            self.generation += 1;
        }
        self.audio_format = Some(format);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_id_rejects_zero() {
        assert!(TrackId::new(0).is_none());
        assert_eq!(TrackId::new(1).unwrap().get(), 1);
    }

    #[test]
    fn codec_config_generation_increments_on_change() {
        let mut info = TrackInfo::new(
            TrackId::new(1).unwrap(),
            TrackKind::Video,
            CodecId::H264,
            TimeBase::DEFAULT,
        );
        assert_eq!(info.generation, 0);
        info.set_codec_config(CodecConfig::AvcC(vec![0x01, 0x42]));
        assert_eq!(info.generation, 1);
        info.set_codec_config(CodecConfig::AvcC(vec![0x01, 0x42]));
        assert_eq!(info.generation, 1);
    }
}

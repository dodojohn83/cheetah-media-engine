//! FLV audio tag parsing and building.

use alloc::vec::Vec;

use cheetah_media_bitstream::aac::AudioSpecificConfig;
use cheetah_media_bitstream::mp3::Mp3Header;
use cheetah_media_types::{
    AudioFormat, ChannelLayout, CodecConfig, CodecId, MediaError, SampleFormat, TrackInfo,
    TrackKind,
};

use crate::FlvError;

/// Audio codec identifiers in the first byte of an FLV audio tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundFormat {
    LinearPcm,
    AdPcm,
    Mp3,
    LinearPcmLe,
    Nellymoser16k,
    Nellymoser8k,
    Nellymoser,
    G711A,
    G711U,
    Aac,
    Speex,
    Mp3_8k,
    DeviceSpecific,
    Unknown(u8),
}

impl SoundFormat {
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::LinearPcm,
            1 => Self::AdPcm,
            2 => Self::Mp3,
            3 => Self::LinearPcmLe,
            4 => Self::Nellymoser16k,
            5 => Self::Nellymoser8k,
            6 => Self::Nellymoser,
            7 => Self::G711A,
            8 => Self::G711U,
            10 => Self::Aac,
            11 => Self::Speex,
            14 => Self::Mp3_8k,
            15 => Self::DeviceSpecific,
            _ => Self::Unknown(v),
        }
    }

    pub const fn to_codec_id(self) -> Option<CodecId> {
        match self {
            Self::Aac => Some(CodecId::Aac),
            Self::Mp3 | Self::Mp3_8k => Some(CodecId::Mp3),
            Self::G711A => Some(CodecId::G711A),
            Self::G711U => Some(CodecId::G711U),
            _ => None,
        }
    }
}

/// Parsed FLV audio tag header (first 1 or 2 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioTagHeader {
    pub sound_format: SoundFormat,
    pub sound_rate: u8,
    pub sound_size: u8,
    pub sound_type: u8,
    /// For AAC: 0 = sequence header, 1 = raw AAC frame.
    pub aac_packet_type: Option<u8>,
    /// Byte length of the header in the tag body.
    pub header_size: usize,
}

impl AudioTagHeader {
    /// Parse an audio tag header from the start of `data`.
    pub fn parse(data: &[u8]) -> Result<Self, FlvError> {
        if data.is_empty() {
            return Err(FlvError::MalformedTag);
        }
        let b0 = data[0];
        let sound_format = SoundFormat::from_u8((b0 >> 4) & 0x0f);
        let sound_rate = (b0 >> 2) & 0x03;
        let sound_size = (b0 >> 1) & 0x01;
        let sound_type = b0 & 0x01;

        if matches!(sound_format, SoundFormat::Aac) {
            if data.len() < 2 {
                return Err(FlvError::MalformedTag);
            }
            Ok(Self {
                sound_format,
                sound_rate,
                sound_size,
                sound_type,
                aac_packet_type: Some(data[1]),
                header_size: 2,
            })
        } else {
            Ok(Self {
                sound_format,
                sound_rate,
                sound_size,
                sound_type,
                aac_packet_type: None,
                header_size: 1,
            })
        }
    }

    /// Build a minimal 1-byte audio tag header for the given codec.
    pub fn build_header_byte(codec: CodecId, sample_rate: u32, channels: u8) -> u8 {
        let format_nibble: u8 = match codec {
            CodecId::Aac => 10,
            CodecId::Mp3 => 2,
            CodecId::G711A => 7,
            CodecId::G711U => 8,
            _ => 15,
        };
        let rate_bits: u8 = if sample_rate <= 8000 {
            0
        } else if sample_rate <= 16_000 {
            1
        } else if sample_rate <= 32_000 {
            2
        } else {
            3
        };
        let sound_size: u8 = if matches!(codec, CodecId::G711A | CodecId::G711U) {
            0
        } else {
            1
        };
        let type_bit: u8 = if channels > 1 { 1 } else { 0 };
        (format_nibble << 4) | (rate_bits << 2) | (sound_size << 1) | type_bit
    }

    /// True if this is an AAC config packet.
    pub fn is_aac_config(self) -> bool {
        matches!(self.sound_format, SoundFormat::Aac) && self.aac_packet_type == Some(0)
    }

    /// True if this is an AAC raw frame.
    pub fn is_aac_raw(self) -> bool {
        matches!(self.sound_format, SoundFormat::Aac) && self.aac_packet_type == Some(1)
    }
}

/// Parse an AAC config tag body and update `track`.
pub fn parse_aac_config(track: &mut TrackInfo, payload: &[u8]) -> Result<(), FlvError> {
    if track.kind != TrackKind::Audio {
        return Err(FlvError::MalformedTag);
    }
    let asc = AudioSpecificConfig::parse(payload).map_err(|_| FlvError::MalformedTag)?;
    track.codec = CodecId::Aac;
    track.set_codec_config(CodecConfig::AacAudioSpecificConfig(asc.build()));
    let channels = ChannelLayout::from_channel_count(u32::from(asc.channel_count));
    // HE-AAC / HE-AAC v2 (object types 5 and 29) output 2048 samples per frame.
    let sample_count = if matches!(asc.audio_object_type, 5 | 29) {
        2048
    } else {
        1024
    };
    let format = AudioFormat {
        sample_format: SampleFormat::S16,
        sample_rate: asc.sampling_frequency,
        channel_layout: channels,
        sample_count,
    };
    track
        .set_audio_format(format)
        .map_err(|_| FlvError::MalformedTag)?;
    Ok(())
}

/// Build an AAC config tag body (2-byte ASC) for `track`.
pub fn build_aac_config(track: &TrackInfo) -> Result<Vec<u8>, FlvError> {
    let bytes = track.codec_config.bytes().ok_or(FlvError::MalformedTag)?;
    let mut out = Vec::with_capacity(2 + bytes.len());
    let header = AudioTagHeader::build_header_byte(
        CodecId::Aac,
        track.audio_format.map(|f| f.sample_rate).unwrap_or(44100),
        track
            .audio_format
            .map(|f| f.channel_layout.channels() as u8)
            .unwrap_or(2),
    );
    out.push(header);
    out.push(0); // AAC sequence header
    out.extend_from_slice(bytes);
    Ok(out)
}

/// Build an AAC raw frame tag body.
pub fn build_aac_raw_frame(track: &TrackInfo, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + payload.len());
    let header = AudioTagHeader::build_header_byte(
        CodecId::Aac,
        track.audio_format.map(|f| f.sample_rate).unwrap_or(44100),
        track
            .audio_format
            .map(|f| f.channel_layout.channels() as u8)
            .unwrap_or(2),
    );
    out.push(header);
    out.push(1); // AAC raw
    out.extend_from_slice(payload);
    out
}

/// Map an FLV `sound_rate` bit field to a sample rate (for non-AAC codecs).
pub const fn sound_rate_to_hz(sound_rate: u8, format: SoundFormat) -> u32 {
    match format {
        SoundFormat::Mp3 | SoundFormat::Mp3_8k => match sound_rate {
            0 => 5500,
            1 => 11025,
            2 => 22050,
            _ => 44100,
        },
        SoundFormat::G711A | SoundFormat::G711U => 8000,
        _ => 44100,
    }
}

/// Derive an `AudioFormat` for MP3/G.711 raw tags from the tag header and payload.
///
/// For AAC the format comes from the AudioSpecificConfig; this helper is used for
/// codecs that do not have a separate config tag and carry all needed parameters
/// in each raw frame.
pub fn audio_format_from_header(ah: &AudioTagHeader, payload: &[u8]) -> Option<AudioFormat> {
    let channels = if ah.sound_type == 1 { 2 } else { 1 };
    match ah.sound_format.to_codec_id()? {
        CodecId::G711A | CodecId::G711U => {
            let sample_count = u32::try_from(payload.len() / channels as usize).ok()?;
            Some(AudioFormat {
                sample_format: if ah.sound_size == 1 {
                    SampleFormat::S16
                } else {
                    SampleFormat::U8
                },
                sample_rate: sound_rate_to_hz(ah.sound_rate, ah.sound_format),
                channel_layout: ChannelLayout::from_channel_count(channels),
                sample_count,
            })
        }
        CodecId::Mp3 => {
            if let Ok(header) = Mp3Header::parse(payload) {
                Some(AudioFormat {
                    sample_format: SampleFormat::S16,
                    sample_rate: header.sample_rate,
                    channel_layout: ChannelLayout::from_channel_count(u32::from(
                        header.channel_count,
                    )),
                    sample_count: u32::from(header.samples_per_frame),
                })
            } else {
                Some(AudioFormat {
                    sample_format: SampleFormat::S16,
                    sample_rate: sound_rate_to_hz(ah.sound_rate, ah.sound_format),
                    channel_layout: ChannelLayout::from_channel_count(channels),
                    sample_count: 1152,
                })
            }
        }
        _ => None,
    }
}

/// Build a generic audio tag body (1-byte header + payload) for MP3/G.711.
pub fn build_audio_raw_frame(
    codec: CodecId,
    sample_rate: u32,
    channels: u8,
    payload: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + payload.len());
    out.push(AudioTagHeader::build_header_byte(
        codec,
        sample_rate,
        channels,
    ));
    out.extend_from_slice(payload);
    out
}

/// Validate that an audio track can be represented in FLV.
pub fn check_audio_track(track: &TrackInfo) -> Result<(), MediaError> {
    match track.codec {
        CodecId::Aac | CodecId::Mp3 | CodecId::G711A | CodecId::G711U => Ok(()),
        _ => Err(MediaError::Unsupported {
            code: 2001,
            context: Some("audio codec not supported by FLV muxer"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_types::{TimeBase, TrackId};

    #[test]
    fn audio_header_aac_config() {
        let header = AudioTagHeader::parse(&[0xaf, 0x00]).unwrap();
        assert!(matches!(header.sound_format, SoundFormat::Aac));
        assert_eq!(header.aac_packet_type, Some(0));
    }

    #[test]
    fn audio_header_mp3() {
        let header = AudioTagHeader::parse(&[0x2a]).unwrap();
        assert!(matches!(header.sound_format, SoundFormat::Mp3));
        assert_eq!(header.sound_rate, 2);
    }

    #[test]
    fn aac_config_round_trip() {
        let mut track = TrackInfo::new(
            TrackId::new(1).unwrap(),
            TrackKind::Audio,
            CodecId::Aac,
            TimeBase::DEFAULT,
        );
        // Typical AAC-LC 44.1 kHz stereo ASC: 0x12 0x10.
        let payload = [0x12, 0x10];
        parse_aac_config(&mut track, &payload).unwrap();
        assert_eq!(track.codec, CodecId::Aac);
        let built = build_aac_config(&track).unwrap();
        assert_eq!(&built[2..], &payload);
    }

    #[test]
    fn g711_audio_format_from_header() {
        // 0x70 -> G.711 A-law, sound rate 0, 8-bit, mono.
        let header = AudioTagHeader::parse(&[0x70]).unwrap();
        let payload = [0u8; 160];
        let fmt = audio_format_from_header(&header, &payload).unwrap();
        assert_eq!(fmt.sample_format, SampleFormat::U8);
        assert_eq!(fmt.sample_rate, 8000);
        assert_eq!(fmt.channel_layout, ChannelLayout::Mono);
        assert_eq!(fmt.sample_count, 160);
    }

    #[test]
    fn mp3_audio_format_from_header() {
        // 0x22 -> MP3, sound rate 0 (5.5 kHz index), 8-bit, mono.
        let header = AudioTagHeader::parse(&[0x22]).unwrap();
        let payload = [0xff, 0xfb, 0x90, 0x64];
        let fmt = audio_format_from_header(&header, &payload).unwrap();
        assert_eq!(fmt.sample_format, SampleFormat::S16);
        assert_eq!(fmt.sample_rate, 44100);
        assert_eq!(fmt.channel_layout.channels(), 2);
        assert_eq!(fmt.sample_count, 1152);
    }
}

//! AAC bitstream helpers: ADTS framing and AudioSpecificConfig.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// AAC sampling frequency table (indices 0-12). Indices 13/14 are explicit.
pub const SAMPLE_RATES: [u32; 13] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

/// AAC channel configuration table.
pub const CHANNEL_COUNTS: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 8];

/// Errors in AAC parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AacError {
    TooShort,
    InvalidSync,
    InvalidSampleRateIndex,
    InvalidChannelConfig,
    InvalidFrameLength,
}

impl core::fmt::Display for AacError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort => write!(f, "AAC data too short"),
            Self::InvalidSync => write!(f, "AAC invalid syncword"),
            Self::InvalidSampleRateIndex => write!(f, "AAC invalid sample rate index"),
            Self::InvalidChannelConfig => write!(f, "AAC invalid channel config"),
            Self::InvalidFrameLength => write!(f, "AAC invalid ADTS frame length"),
        }
    }
}

/// An AAC AudioSpecificConfig (ASC) for ADTS-free streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSpecificConfig {
    pub audio_object_type: u8,
    pub sampling_frequency_index: u8,
    pub sampling_frequency: u32,
    pub channel_configuration: u8,
    pub channel_count: u8,
}

impl AudioSpecificConfig {
    /// Parse an ASC from a byte slice (audioObjectType 2 only here).
    pub fn parse(data: &[u8]) -> Result<Self, AacError> {
        if data.len() < 2 {
            return Err(AacError::TooShort);
        }
        let b0 = data[0];
        let b1 = data[1];
        let audio_object_type = (b0 >> 3) & 0x1f;
        let sampling_frequency_index = ((b0 & 0x07) << 1) | ((b1 >> 7) & 0x01);
        let channel_configuration = (b1 >> 3) & 0x0f;
        if sampling_frequency_index as usize >= SAMPLE_RATES.len() {
            return Err(AacError::InvalidSampleRateIndex);
        }
        if (channel_configuration as usize) >= CHANNEL_COUNTS.len() {
            return Err(AacError::InvalidChannelConfig);
        }
        Ok(Self {
            audio_object_type,
            sampling_frequency_index,
            sampling_frequency: SAMPLE_RATES[sampling_frequency_index as usize],
            channel_configuration,
            channel_count: CHANNEL_COUNTS[channel_configuration as usize],
        })
    }

    /// Build a 2-byte ASC from the fields.
    pub fn build(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(2);
        let b0 =
            ((self.audio_object_type & 0x1f) << 3) | ((self.sampling_frequency_index >> 1) & 0x07);
        let b1 = ((self.sampling_frequency_index & 0x01) << 7)
            | ((self.channel_configuration & 0x0f) << 3);
        out.push(b0);
        out.push(b1);
        out
    }
}

/// An ADTS frame header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdtsHeader {
    pub id: u8,
    pub layer: u8,
    pub protection_absent: bool,
    pub profile: u8,
    pub sampling_frequency_index: u8,
    pub sampling_frequency: u32,
    pub private_bit: u8,
    pub channel_configuration: u8,
    pub channel_count: u8,
    pub frame_length: u16,
    pub buffer_fullness: u16,
    pub number_of_raw_data_blocks_in_frame: u8,
    pub crc_present: bool,
    pub samples_per_frame: u16,
    pub duration_ms: u32,
}

impl AdtsHeader {
    /// Try to parse an ADTS header at the start of `data`.
    pub fn parse(data: &[u8]) -> Result<Self, AacError> {
        if data.len() < 7 {
            return Err(AacError::TooShort);
        }
        let b0 = data[0];
        let b1 = data[1];
        let syncword = ((b0 as u16) << 4) | ((b1 >> 4) as u16);
        if syncword != 0xfff {
            return Err(AacError::InvalidSync);
        }
        let id = (b1 >> 3) & 0x01;
        let layer = (b1 >> 1) & 0x03;
        let protection_absent = (b1 & 0x01) != 0;

        let b2 = data[2];
        let profile = (b2 >> 6) & 0x03;
        let sampling_frequency_index = (b2 >> 2) & 0x0f;
        let private_bit = (b2 >> 1) & 0x01;
        let channel_configuration = ((b2 & 0x01) << 2) | ((data[3] >> 6) & 0x03);

        let b3 = data[3];
        let b4 = data[4];
        let b5 = data[5];
        let b6 = data[6];
        let frame_length = (((b3 & 0x03) as u16) << 11) | ((b4 as u16) << 3) | ((b5 >> 5) as u16);
        let buffer_fullness = (((b5 & 0x1f) as u16) << 6) | ((b6 >> 2) as u16);
        let number_of_raw_data_blocks_in_frame = b6 & 0x03;

        if sampling_frequency_index as usize >= SAMPLE_RATES.len() {
            return Err(AacError::InvalidSampleRateIndex);
        }
        let sampling_frequency = SAMPLE_RATES[sampling_frequency_index as usize];
        if (channel_configuration as usize) >= CHANNEL_COUNTS.len() {
            return Err(AacError::InvalidChannelConfig);
        }
        let channel_count = CHANNEL_COUNTS[channel_configuration as usize];

        let header_size = if protection_absent { 7 } else { 9 };
        if frame_length < header_size {
            return Err(AacError::InvalidFrameLength);
        }

        let samples_per_frame = 1024u16 * (1 + u16::from(number_of_raw_data_blocks_in_frame));
        let duration_ms = (samples_per_frame as u64 * 1000 / sampling_frequency as u64) as u32;

        Ok(Self {
            id,
            layer,
            protection_absent,
            profile,
            sampling_frequency_index,
            sampling_frequency,
            private_bit,
            channel_configuration,
            channel_count,
            frame_length,
            buffer_fullness,
            number_of_raw_data_blocks_in_frame,
            crc_present: !protection_absent,
            samples_per_frame,
            duration_ms,
        })
    }

    /// Header size in bytes (7 without CRC, 9 with CRC).
    pub fn header_size(&self) -> usize {
        if self.crc_present { 9 } else { 7 }
    }

    /// Build a 7-byte ADTS header without CRC.
    pub fn build(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(7);
        out.push(0xff);
        out.push(
            0xf0 | ((self.id & 0x01) << 3)
                | ((self.layer & 0x03) << 1)
                | (if self.protection_absent { 0x01 } else { 0x00 }),
        );
        out.push(
            ((self.profile & 0x03) << 6)
                | ((self.sampling_frequency_index & 0x0f) << 2)
                | ((self.private_bit & 0x01) << 1)
                | ((self.channel_configuration >> 2) & 0x01),
        );
        out.push(
            ((self.channel_configuration & 0x03) << 6) | (((self.frame_length >> 11) & 0x03) as u8),
        );
        out.push(((self.frame_length >> 3) & 0x00ff) as u8);
        out.push(
            (((self.frame_length & 0x07) as u8) << 5)
                | (((self.buffer_fullness >> 6) & 0x1f) as u8),
        );
        out.push(
            (((self.buffer_fullness & 0x3f) as u8) << 2)
                | (self.number_of_raw_data_blocks_in_frame & 0x03),
        );
        out
    }

    /// RFC 6381 codec string for AAC-LC.
    pub fn codec_string(&self) -> String {
        alloc::format!("mp4a.40.{}", self.profile + 1)
    }
}

/// Split an AAC ADTS stream into individual frames. Each frame is a borrowed slice.
pub fn split_adts(data: &[u8]) -> Result<Vec<&[u8]>, AacError> {
    let mut frames = Vec::new();
    let mut pos = 0usize;
    loop {
        let header_end = pos.checked_add(7).ok_or(AacError::InvalidFrameLength)?;
        if header_end > data.len() {
            break;
        }
        let header = AdtsHeader::parse(&data[pos..])?;
        let header_size = header.header_size();
        let frame_length = header.frame_length as usize;
        if frame_length < header_size {
            return Err(AacError::InvalidFrameLength);
        }
        let end = pos
            .checked_add(frame_length)
            .ok_or(AacError::InvalidFrameLength)?;
        if end > data.len() {
            return Err(AacError::TooShort);
        }
        frames.push(&data[pos..end]);
        pos = end;
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_specific_config_round_trip() {
        let cfg = AudioSpecificConfig {
            audio_object_type: 2,
            sampling_frequency_index: 4,
            sampling_frequency: 44100,
            channel_configuration: 2,
            channel_count: 2,
        };
        let bytes = cfg.build();
        let parsed = AudioSpecificConfig::parse(&bytes).unwrap();
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn adts_header_build_and_parse() {
        let header = AdtsHeader {
            id: 0,
            layer: 0,
            protection_absent: true,
            profile: 1,
            sampling_frequency_index: 4,
            sampling_frequency: 44100,
            private_bit: 0,
            channel_configuration: 2,
            channel_count: 2,
            frame_length: 100,
            buffer_fullness: 0,
            number_of_raw_data_blocks_in_frame: 0,
            crc_present: false,
            samples_per_frame: 1024,
            duration_ms: 23,
        };
        let bytes = header.build();
        let parsed = AdtsHeader::parse(&bytes).unwrap();
        assert_eq!(parsed.frame_length, 100);
        assert_eq!(parsed.sampling_frequency, 44100);
        assert_eq!(parsed.channel_count, 2);
    }

    #[test]
    fn adts_header_build_respects_protection_absent() {
        let header = AdtsHeader {
            id: 0,
            layer: 0,
            protection_absent: false,
            profile: 1,
            sampling_frequency_index: 4,
            sampling_frequency: 44100,
            private_bit: 0,
            channel_configuration: 2,
            channel_count: 2,
            frame_length: 100,
            buffer_fullness: 0,
            number_of_raw_data_blocks_in_frame: 0,
            crc_present: true,
            samples_per_frame: 1024,
            duration_ms: 23,
        };
        let bytes = header.build();
        let parsed = AdtsHeader::parse(&bytes).unwrap();
        assert!(!parsed.protection_absent);
        assert!(parsed.crc_present);
    }

    #[test]
    fn split_adts_rejects_zero_frame_length() {
        // Valid 7-byte ADTS header except the 13-bit frame_length field is zero.
        let mut header = AdtsHeader {
            id: 0,
            layer: 0,
            protection_absent: true,
            profile: 1,
            sampling_frequency_index: 4,
            sampling_frequency: 44100,
            private_bit: 0,
            channel_configuration: 2,
            channel_count: 2,
            frame_length: 0,
            buffer_fullness: 0,
            number_of_raw_data_blocks_in_frame: 0,
            crc_present: false,
            samples_per_frame: 1024,
            duration_ms: 23,
        };
        let bytes = header.build();
        assert!(split_adts(&bytes).is_err());

        // Also reject a frame length smaller than the header size.
        header.frame_length = 6;
        let bytes = header.build();
        assert!(matches!(
            split_adts(&bytes),
            Err(AacError::InvalidFrameLength)
        ));
    }
}

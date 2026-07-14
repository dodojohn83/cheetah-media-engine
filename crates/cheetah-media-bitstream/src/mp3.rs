//! MP3 (MPEG-1/2 Layer III) frame header parsing.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// MPEG versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpegVersion {
    V25,
    Reserved,
    V2,
    V1,
}

impl MpegVersion {
    pub const fn from_bits(bits: u8) -> Self {
        match bits {
            0b00 => Self::V25,
            0b01 => Self::Reserved,
            0b10 => Self::V2,
            0b11 => Self::V1,
            _ => Self::Reserved,
        }
    }

    pub const fn samples_per_frame(&self, layer: Layer) -> u16 {
        match (self, layer) {
            (Self::V1, Layer::I) => 384,
            (Self::V1, Layer::II) | (Self::V1, Layer::III) => 1152,
            (Self::V2, Layer::I) => 384,
            (Self::V2, Layer::II) => 1152,
            (Self::V2, Layer::III) => 576,
            (Self::V25, Layer::I) => 384,
            (Self::V25, Layer::II) => 1152,
            (Self::V25, Layer::III) => 576,
            _ => 0,
        }
    }
}

/// MPEG layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Reserved,
    III,
    II,
    I,
}

impl Layer {
    pub const fn from_bits(bits: u8) -> Self {
        match bits {
            0b00 => Self::Reserved,
            0b01 => Self::III,
            0b10 => Self::II,
            0b11 => Self::I,
            _ => Self::Reserved,
        }
    }
}

/// Channel mode from the 2-bit header field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelMode {
    Stereo,
    JointStereo,
    DualChannel,
    Mono,
}

impl ChannelMode {
    pub const fn from_bits(bits: u8) -> Self {
        match bits {
            0b00 => Self::Stereo,
            0b01 => Self::JointStereo,
            0b10 => Self::DualChannel,
            0b11 => Self::Mono,
            _ => Self::Stereo,
        }
    }

    pub const fn channel_count(&self) -> u8 {
        match self {
            Self::Mono => 1,
            _ => 2,
        }
    }
}

/// Errors in MP3 header parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mp3Error {
    TooShort,
    InvalidSync,
    InvalidVersion,
    InvalidLayer,
    InvalidBitrate,
    InvalidSampleRate,
}

impl core::fmt::Display for Mp3Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort => write!(f, "MP3 data too short"),
            Self::InvalidSync => write!(f, "MP3 invalid syncword"),
            Self::InvalidVersion => write!(f, "MP3 invalid MPEG version"),
            Self::InvalidLayer => write!(f, "MP3 invalid layer"),
            Self::InvalidBitrate => write!(f, "MP3 invalid bitrate index"),
            Self::InvalidSampleRate => write!(f, "MP3 invalid sample rate index"),
        }
    }
}

// Bitrate table in kbps: [version][layer][bitrate_index].
const BITRATE_TABLE: [[[u16; 15]; 4]; 2] = [
    [
        // MPEG-1
        [
            0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
        ], // Layer I
        [
            0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
        ], // Layer II
        [
            0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
        ], // Layer III
        [0; 15], // reserved
    ],
    [
        // MPEG-2 / 2.5
        [
            0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
        ], // Layer I
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160], // Layer II
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160], // Layer III
        [0; 15],
    ],
];

// Sample rate table: [version_index][sample_rate_index].
const SAMPLE_RATE_TABLE: [[u32; 4]; 3] = [
    [44100, 48000, 32000, 0], // MPEG-1
    [22050, 24000, 16000, 0], // MPEG-2
    [11025, 12000, 8000, 0],  // MPEG-2.5
];

/// A parsed MP3 frame header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mp3Header {
    pub version: MpegVersion,
    pub layer: Layer,
    pub crc_protected: bool,
    pub bitrate_kbps: u16,
    pub sample_rate_index: u8,
    pub sample_rate: u32,
    pub padding: bool,
    pub channel_mode: ChannelMode,
    pub channel_count: u8,
    pub frame_length: u16,
    pub samples_per_frame: u16,
    pub duration_ms: u32,
}

impl Mp3Header {
    /// Parse the 4-byte MP3 frame header.
    pub fn parse(data: &[u8]) -> Result<Self, Mp3Error> {
        if data.len() < 4 {
            return Err(Mp3Error::TooShort);
        }
        let b0 = data[0];
        let b1 = data[1];
        let b2 = data[2];
        let b3 = data[3];

        let syncword = (b0 as u16) << 3 | ((b1 >> 5) as u16);
        if syncword != 0x7ff {
            return Err(Mp3Error::InvalidSync);
        }

        let version_bits = (b1 >> 3) & 0x03;
        let version = MpegVersion::from_bits(version_bits);
        if matches!(version, MpegVersion::Reserved) {
            return Err(Mp3Error::InvalidVersion);
        }
        let layer_bits = (b1 >> 1) & 0x03;
        let layer = Layer::from_bits(layer_bits);
        if matches!(layer, Layer::Reserved) {
            return Err(Mp3Error::InvalidLayer);
        }
        let crc_protected = (b1 & 0x01) == 0;

        let bitrate_index = (b2 >> 4) & 0x0f;
        if bitrate_index == 0 || bitrate_index == 0x0f {
            return Err(Mp3Error::InvalidBitrate);
        }
        let sample_rate_index = (b2 >> 2) & 0x03;
        if sample_rate_index == 0x03 {
            return Err(Mp3Error::InvalidSampleRate);
        }
        let sample_rate = {
            let v = match version {
                MpegVersion::V1 => 0,
                MpegVersion::V2 => 1,
                MpegVersion::V25 => 2,
                MpegVersion::Reserved => 0,
            };
            SAMPLE_RATE_TABLE[v as usize][sample_rate_index as usize]
        };
        let padding = ((b2 >> 1) & 0x01) != 0;

        let channel_mode = ChannelMode::from_bits((b3 >> 6) & 0x03);
        let channel_count = channel_mode.channel_count();

        let bitrate_kbps = {
            let v = if matches!(version, MpegVersion::V1) {
                0
            } else {
                1
            };
            let l = match layer {
                Layer::I => 0,
                Layer::II => 1,
                Layer::III => 2,
                Layer::Reserved => 3,
            };
            BITRATE_TABLE[v][l][bitrate_index as usize]
        };

        let samples_per_frame = version.samples_per_frame(layer);
        let slot_size = if matches!(layer, Layer::I) { 4 } else { 1 };
        let mut frame_length =
            (samples_per_frame as u32 * u32::from(bitrate_kbps) * 1000 / 8 / sample_rate)
                * slot_size as u32;
        if padding {
            frame_length += slot_size as u32;
        }

        let duration_ms = (samples_per_frame as u64 * 1000 / sample_rate as u64) as u32;

        Ok(Self {
            version,
            layer,
            crc_protected,
            bitrate_kbps,
            sample_rate_index,
            sample_rate,
            padding,
            channel_mode,
            channel_count,
            frame_length: frame_length as u16,
            samples_per_frame,
            duration_ms,
        })
    }

    /// RFC 6381 codec string.
    pub fn codec_string(&self) -> String {
        String::from("mp3")
    }
}

/// Locate the next MP3 frame header in `data` starting at `offset`.
pub fn find_next_frame(data: &[u8], offset: usize) -> Option<usize> {
    if data.len() < offset + 4 {
        return None;
    }
    (offset..=data.len() - 4).find(|&i| data[i] == 0xff && (data[i + 1] & 0xe0) == 0xe0)
}

/// Split an MP3 stream into frame slices. If the final frame is truncated it is omitted.
pub fn split_mp3(data: &[u8]) -> Result<Vec<&[u8]>, Mp3Error> {
    let mut frames = Vec::new();
    let mut pos = 0usize;
    while let Some(start) = find_next_frame(data, pos) {
        if start + 4 > data.len() {
            break;
        }
        let header = Mp3Header::parse(&data[start..])?;
        let end = start + header.frame_length as usize;
        if end > data.len() {
            break;
        }
        frames.push(&data[start..end]);
        pos = end;
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mp3_header_calculation() {
        // 0xfffb 9064 -> MPEG-1 Layer III, 128kbps, 44.1kHz, no padding, joint stereo
        let data = [0xff, 0xfb, 0x90, 0x64];
        let header = Mp3Header::parse(&data).unwrap();
        assert_eq!(header.version, MpegVersion::V1);
        assert_eq!(header.layer, Layer::III);
        assert_eq!(header.sample_rate, 44100);
        assert_eq!(header.bitrate_kbps, 128);
        assert_eq!(header.channel_count, 2);
        assert_eq!(header.samples_per_frame, 1152);
        assert_eq!(header.frame_length, 417);
    }

    #[test]
    fn mp3_header_invalid_sync() {
        let data = [0x00, 0x00, 0x00, 0x00];
        assert!(Mp3Header::parse(&data).is_err());
    }

    #[test]
    fn mp3_mpeg2_layer_iii_samples_per_frame() {
        // 0xfff2 9064 -> MPEG-2 Layer III, 80kbps, 22.05kHz, joint stereo.
        let data = [0xff, 0xf2, 0x90, 0x64];
        let header = Mp3Header::parse(&data).unwrap();
        assert_eq!(header.version, MpegVersion::V2);
        assert_eq!(header.layer, Layer::III);
        assert_eq!(header.samples_per_frame, 576);
        assert_eq!(header.frame_length, 261);
    }
}

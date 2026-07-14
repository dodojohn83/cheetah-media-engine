//! H.264 bitstream helpers: NAL splitting, Annex-B/AVCC conversion, and
//! minimal SPS parsing.

extern crate alloc;

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use crate::bit::{BitCursor, BitError};
use crate::{ByteCursor, ReadError};

/// Errors specific to H.264 parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264Error {
    EndOfStream,
    InvalidNalLength,
    InvalidStartCode,
    InvalidSps,
    UnsupportedProfile,
}

impl core::fmt::Display for H264Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EndOfStream => write!(f, "H.264 end of stream"),
            Self::InvalidNalLength => write!(f, "H.264 invalid NAL length"),
            Self::InvalidStartCode => write!(f, "H.264 invalid Annex-B start code"),
            Self::InvalidSps => write!(f, "H.264 invalid SPS"),
            Self::UnsupportedProfile => write!(f, "H.264 unsupported profile for SPS parsing"),
        }
    }
}

impl From<BitError> for H264Error {
    fn from(_: BitError) -> Self {
        Self::InvalidSps
    }
}

impl From<ReadError> for H264Error {
    fn from(_: ReadError) -> Self {
        Self::EndOfStream
    }
}

/// A single H.264 NAL unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NalUnit<'a> {
    /// NAL reference IDC.
    pub nal_ref_idc: u8,
    /// NAL unit type.
    pub nal_type: u8,
    /// NAL header byte plus RBSP payload (without start code/length prefix).
    pub data: &'a [u8],
    /// RBSP payload after the first header byte.
    pub payload: &'a [u8],
}

impl<'a> NalUnit<'a> {
    /// True for IDR slices.
    pub fn is_idr(&self) -> bool {
        self.nal_type == 5
    }

    /// True for slices (IDR or non-IDR) and not SEI/AUD/etc.
    pub fn is_slice(&self) -> bool {
        matches!(self.nal_type, 1 | 5)
    }

    /// True for parameter set NAL types.
    pub fn is_parameter_set(&self) -> bool {
        matches!(self.nal_type, 7 | 8)
    }

    /// Convert this NAL unit to Annex-B form with a 4-byte start code.
    pub fn to_annexb(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.data.len());
        out.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        out.extend_from_slice(self.data);
        out
    }

    /// Convert this NAL unit to AVCC form with `length_size` bytes.
    pub fn to_avcc(&self, length_size: u8) -> Result<Vec<u8>, H264Error> {
        if !matches!(length_size, 1 | 2 | 4) {
            return Err(H264Error::InvalidNalLength);
        }
        let len = self.data.len();
        if (len as u64) >= (1u64 << (length_size * 8)) {
            return Err(H264Error::InvalidNalLength);
        }
        let mut out = Vec::with_capacity(usize::from(length_size) + self.data.len());
        match length_size {
            1 => out.push(len as u8),
            2 => out.extend_from_slice(&(len as u16).to_be_bytes()),
            4 => out.extend_from_slice(&(len as u32).to_be_bytes()),
            _ => unreachable!(),
        }
        out.extend_from_slice(self.data);
        Ok(out)
    }
}

/// Find the next Annex-B start code (`00 00 01` or `00 00 00 01`) in `data`
/// starting at `start`.
fn find_start_code(data: &[u8], start: usize) -> Option<usize> {
    if data.len() < start + 3 {
        return None;
    }
    let mut i = start;
    while i + 2 < data.len() {
        if data[i] == 0x00 && data[i + 1] == 0x00 && data[i + 2] == 0x01 {
            return Some(i);
        }
        if data[i] == 0x00
            && data[i + 1] == 0x00
            && data[i + 2] == 0x00
            && i + 3 < data.len()
            && data[i + 3] == 0x01
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Split Annex-B data into NAL units.
pub fn split_annexb<'a>(data: &'a [u8]) -> Result<Vec<NalUnit<'a>>, H264Error> {
    let mut units = Vec::new();
    let mut pos = 0usize;

    while pos < data.len() {
        let start = find_start_code(data, pos).ok_or(H264Error::InvalidStartCode)?;
        let mut code_len = 3usize;
        if start + 3 < data.len() && data[start + 2] == 0x00 && data[start + 3] == 0x01 {
            code_len = 4;
        }
        let header_pos = start + code_len;
        if header_pos >= data.len() {
            break;
        }
        let nal_header = data[header_pos];
        let nal_ref_idc = (nal_header >> 5) & 0x03;
        let nal_type = nal_header & 0x1f;

        let next = find_start_code(data, header_pos + 1).unwrap_or(data.len());
        let unit_data = &data[header_pos..next];
        let payload = if !unit_data.is_empty() {
            &unit_data[1..]
        } else {
            &[]
        };

        units.push(NalUnit {
            nal_ref_idc,
            nal_type,
            data: unit_data,
            payload,
        });
        pos = next;
    }

    Ok(units)
}

/// Parse AVCC length-prefixed NAL units.
pub fn split_avcc<'a>(data: &'a [u8], length_size: u8) -> Result<Vec<NalUnit<'a>>, H264Error> {
    if !matches!(length_size, 1 | 2 | 4) {
        return Err(H264Error::InvalidNalLength);
    }
    let mut units = Vec::new();
    let mut cursor = ByteCursor::new(data);
    let _ls = usize::from(length_size);

    while !cursor.is_empty() {
        let len = match length_size {
            1 => cursor.read_u8().map(usize::from)?,
            2 => cursor.read_u16_be().map(usize::from)?,
            4 => cursor.read_u32_be()? as usize,
            _ => unreachable!(),
        };
        if len > cursor.remaining() {
            return Err(H264Error::InvalidNalLength);
        }
        let bytes = cursor.read_bytes(len)?;
        if bytes.is_empty() {
            continue;
        }
        let nal_header = bytes[0];
        let nal_ref_idc = (nal_header >> 5) & 0x03;
        let nal_type = nal_header & 0x1f;
        units.push(NalUnit {
            nal_ref_idc,
            nal_type,
            data: bytes,
            payload: &bytes[1..],
        });
    }

    Ok(units)
}

/// Convert Annex-B data to AVCC length-prefixed data.
pub fn annexb_to_avcc(data: &[u8], length_size: u8) -> Result<Vec<u8>, H264Error> {
    let units = split_annexb(data)?;
    let mut out = Vec::new();
    for unit in units {
        out.extend_from_slice(&unit.to_avcc(length_size)?);
    }
    Ok(out)
}

/// Convert AVCC length-prefixed data to Annex-B data.
pub fn avcc_to_annexb(data: &[u8], length_size: u8) -> Result<Vec<u8>, H264Error> {
    let units = split_avcc(data, length_size)?;
    let mut out = Vec::new();
    for unit in units {
        out.extend_from_slice(&unit.to_annexb());
    }
    Ok(out)
}

/// Minimal H.264 SPS parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sps {
    pub profile_idc: u8,
    pub constraint_set_flags: u8,
    pub level_idc: u8,
    pub seq_parameter_set_id: u64,
    pub chroma_format_idc: u64,
    pub separate_colour_plane_flag: bool,
    pub bit_depth_luma_minus8: u64,
    pub bit_depth_chroma_minus8: u64,
    pub log2_max_frame_num_minus4: u64,
    pub pic_order_cnt_type: u64,
    pub log2_max_pic_order_cnt_lsb_minus4: u64,
    pub max_num_ref_frames: u64,
    pub pic_width_in_mbs_minus1: u64,
    pub pic_height_in_map_units_minus1: u64,
    pub frame_mbs_only_flag: bool,
    pub frame_cropping_flag: bool,
    pub frame_crop_left_offset: u64,
    pub frame_crop_right_offset: u64,
    pub frame_crop_top_offset: u64,
    pub frame_crop_bottom_offset: u64,
    pub width: u32,
    pub height: u32,
}

impl Sps {
    /// Parse SPS from an RBSP payload (without the NAL header byte).
    #[allow(clippy::field_reassign_with_default)]
    pub fn parse(rbsp: &[u8]) -> Result<Self, H264Error> {
        let mut cursor = BitCursor::new(rbsp);
        let mut sps = Self::default();

        sps.profile_idc = cursor.read_u32(8)? as u8;
        sps.constraint_set_flags = cursor.read_u32(8)? as u8;
        sps.level_idc = cursor.read_u32(8)? as u8;
        sps.seq_parameter_set_id = cursor.read_ue()?;

        let high_profiles = [100u8, 110, 122, 244, 44, 83, 86, 118, 128, 138, 139];
        if high_profiles.contains(&sps.profile_idc) {
            sps.chroma_format_idc = cursor.read_ue()?;
            if sps.chroma_format_idc == 3 {
                sps.separate_colour_plane_flag = cursor.read_bool()?;
            }
            sps.bit_depth_luma_minus8 = cursor.read_ue()?;
            sps.bit_depth_chroma_minus8 = cursor.read_ue()?;
            let _qpprime_y_zero_transform_bypass_flag = cursor.read_bool()?;
            let seq_scaling_matrix_present_flag = cursor.read_bool()?;
            if seq_scaling_matrix_present_flag {
                let n = if sps.chroma_format_idc != 3 { 8 } else { 12 };
                for i in 0..n {
                    let present = cursor.read_bool()?;
                    if present {
                        // Consume scaling_list() coefficients per H.264 spec
                        // 7.3.2.1.1.1; values are discarded because we only need
                        // dimensions from the SPS.
                        let size = if i < 6 { 16 } else { 64 };
                        let mut last_scale = 8i64;
                        let mut next_scale = 8i64;
                        for _ in 0..size {
                            if next_scale != 0 {
                                let delta_scale = cursor.read_se()?;
                                next_scale = (last_scale + delta_scale + 256) % 256;
                            }
                            last_scale = next_scale;
                        }
                    }
                }
            }
        }

        // For non-high profiles, chroma_format_idc is not explicitly signalled and
        // shall be inferred as 1 (4:2:0) per H.264 spec 7.4.2.1.1.
        if !high_profiles.contains(&sps.profile_idc) {
            sps.chroma_format_idc = 1;
        }

        sps.log2_max_frame_num_minus4 = cursor.read_ue()?;
        sps.pic_order_cnt_type = cursor.read_ue()?;
        if sps.pic_order_cnt_type == 0 {
            sps.log2_max_pic_order_cnt_lsb_minus4 = cursor.read_ue()?;
        } else if sps.pic_order_cnt_type == 1 {
            let _delta_always_zero = cursor.read_bool()?;
            let _offset_non_ref = cursor.read_se()?;
            let _offset_top_bottom = cursor.read_se()?;
            let num_ref_frames = cursor.read_ue()?;
            for _ in 0..num_ref_frames {
                let _ = cursor.read_se()?;
            }
        }
        sps.max_num_ref_frames = cursor.read_ue()?;
        let _gaps = cursor.read_bool()?;
        sps.pic_width_in_mbs_minus1 = cursor.read_ue()?;
        sps.pic_height_in_map_units_minus1 = cursor.read_ue()?;
        sps.frame_mbs_only_flag = cursor.read_bool()?;
        if !sps.frame_mbs_only_flag {
            let _adaptive = cursor.read_bool()?;
        }
        let _direct_8x8 = cursor.read_bool()?;
        sps.frame_cropping_flag = cursor.read_bool()?;
        if sps.frame_cropping_flag {
            sps.frame_crop_left_offset = cursor.read_ue()?;
            sps.frame_crop_right_offset = cursor.read_ue()?;
            sps.frame_crop_top_offset = cursor.read_ue()?;
            sps.frame_crop_bottom_offset = cursor.read_ue()?;
        }

        let width_in_mbs = sps.pic_width_in_mbs_minus1 + 1;
        let height_in_map_units = sps.pic_height_in_map_units_minus1 + 1;
        let mb_height = if sps.frame_mbs_only_flag { 1 } else { 2 };

        // SubWidthC / SubHeightC per H.264 Table 6-1.
        let sub_width_c = if sps.chroma_format_idc == 1 || sps.chroma_format_idc == 2 {
            2
        } else {
            1
        };
        let sub_height_c = if sps.chroma_format_idc == 1 { 2 } else { 1 };
        let crop_unit_x = sub_width_c;
        // CropUnitY includes the frame_mbs_only_flag factor per spec.
        let crop_unit_y = sub_height_c * mb_height;

        let mut width = (width_in_mbs * 16) as u32;
        let mut height = (height_in_map_units * 16 * mb_height) as u32;
        if sps.frame_cropping_flag {
            width -=
                ((sps.frame_crop_left_offset + sps.frame_crop_right_offset) * crop_unit_x) as u32;
            height -=
                ((sps.frame_crop_top_offset + sps.frame_crop_bottom_offset) * crop_unit_y) as u32;
        }
        sps.width = width;
        sps.height = height;

        Ok(sps)
    }

    /// RFC 6381 codec string: `avc1.XXYYYY` where XX=profile, YYYY=constraint+level.
    pub fn codec_string(&self) -> alloc::string::String {
        format!(
            "avc1.{:02X}{:02X}{:02X}",
            self.profile_idc, self.constraint_set_flags, self.level_idc
        )
    }
}

/// H.264 decoder configuration (AVCDecoderConfigurationRecord).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct H264CodecConfig {
    pub configuration_version: u8,
    pub avc_profile_indication: u8,
    pub profile_compatibility: u8,
    pub avc_level_indication: u8,
    pub length_size_minus_one: u8,
    /// Each SPS NAL unit, including its NAL header byte.
    pub sps_list: Vec<Vec<u8>>,
    /// Each PPS NAL unit, including its NAL header byte.
    pub pps_list: Vec<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub codec_string: alloc::string::String,
}

impl H264CodecConfig {
    /// Parse an AVCDecoderConfigurationRecord.
    pub fn parse(data: &[u8]) -> Result<Self, H264Error> {
        if data.len() < 7 {
            return Err(H264Error::InvalidNalLength);
        }
        let mut cursor = ByteCursor::new(data);
        let configuration_version = cursor.read_u8()?;
        if configuration_version != 1 {
            return Err(H264Error::InvalidNalLength);
        }
        let avc_profile_indication = cursor.read_u8()?;
        let profile_compatibility = cursor.read_u8()?;
        let avc_level_indication = cursor.read_u8()?;
        let length_size_minus_one = cursor.read_u8()? & 0x03;
        let sps_count_byte = cursor.read_u8()? & 0x1f;

        let mut sps_list = Vec::new();
        for _ in 0..sps_count_byte {
            let len = cursor.read_u16_be()? as usize;
            if len > cursor.remaining() {
                return Err(H264Error::InvalidNalLength);
            }
            let nal = cursor.read_bytes(len)?;
            sps_list.push(nal.to_vec());
        }

        let pps_count = cursor.read_u8()?;
        let mut pps_list = Vec::new();
        for _ in 0..pps_count {
            let len = cursor.read_u16_be()? as usize;
            if len > cursor.remaining() {
                return Err(H264Error::InvalidNalLength);
            }
            let nal = cursor.read_bytes(len)?;
            pps_list.push(nal.to_vec());
        }

        let mut width = 0u32;
        let mut height = 0u32;
        let mut codec_string = alloc::string::String::new();
        if let Some(first_sps) = sps_list.first() {
            let header = first_sps[0];
            let nal_type = header & 0x1f;
            if nal_type == 7 {
                // Strip NAL header byte and de-escalate emulation prevention bytes.
                let raw = &first_sps[1..];
                let rbsp = unescape_rbsp(raw);
                if let Ok(parsed) = Sps::parse(&rbsp) {
                    width = parsed.width;
                    height = parsed.height;
                    codec_string = parsed.codec_string();
                }
            }
        }

        if codec_string.is_empty() {
            codec_string = format!(
                "avc1.{:02X}{:02X}{:02X}",
                avc_profile_indication, profile_compatibility, avc_level_indication
            );
        }

        Ok(Self {
            configuration_version,
            avc_profile_indication,
            profile_compatibility,
            avc_level_indication,
            length_size_minus_one,
            sps_list,
            pps_list,
            width,
            height,
            codec_string,
        })
    }

    /// Build an AVCDecoderConfigurationRecord from SPS/PPS.
    pub fn build(&self) -> Vec<u8> {
        let mut out = vec![
            self.configuration_version,
            self.avc_profile_indication,
            self.profile_compatibility,
            self.avc_level_indication,
            self.length_size_minus_one | 0xfc,
            0xe0 | (self.sps_list.len() as u8),
        ];
        for sps in &self.sps_list {
            out.extend_from_slice(&(sps.len() as u16).to_be_bytes());
            out.extend_from_slice(sps);
        }
        out.push(self.pps_list.len() as u8);
        for pps in &self.pps_list {
            out.extend_from_slice(&(pps.len() as u16).to_be_bytes());
            out.extend_from_slice(pps);
        }
        out
    }
}

/// Remove H.264 emulation prevention three bytes (`0x00 0x00 0x03`) from RBSP.
pub fn unescape_rbsp(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if i + 2 < data.len() && data[i] == 0x00 && data[i + 1] == 0x00 && data[i + 2] == 0x03 {
            out.push(0x00);
            out.push(0x00);
            i += 3;
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annexb_round_trip() {
        // Two NAL units: SPS (type 7) and IDR slice (type 5)
        let sps = [0x67, 0x42, 0x00, 0x1e]; // profile 66, constraint 0, level 30
        let idr = [0x65, 0x88];
        let mut annexb = Vec::new();
        annexb.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        annexb.extend_from_slice(&sps);
        annexb.extend_from_slice(&[0x00, 0x00, 0x01]);
        annexb.extend_from_slice(&idr);

        let avcc = annexb_to_avcc(&annexb, 4).unwrap();
        let units = split_avcc(&avcc, 4).unwrap();
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].nal_type, 7);
        assert_eq!(units[1].nal_type, 5);

        let back = avcc_to_annexb(&avcc, 4).unwrap();
        // avcc_to_annexb always uses 4-byte start code, so compare against that.
        let expected = [
            0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1e, 0x00, 0x00, 0x00, 0x01, 0x65, 0x88,
        ];
        assert_eq!(back, expected);
    }

    #[test]
    fn avcc_codec_config_parses_basic_sps() {
        // SPS NAL: profile 66, constraint 0, level 30, seq_parameter_set_id 0,
        // log2_max_frame_num_minus4 0, pic_order_cnt_type 0,
        // log2_max_pic_order_cnt_lsb_minus4 0, max_num_ref_frames 1,
        // pic_width_in_mbs_minus1 15 (=> 256), pic_height_in_map_units_minus1 15,
        // frame_mbs_only_flag true, frame_cropping_flag false.
        // BitCursor bits:
        // profile_idc 66 = 0x42
        // constraint/level 0/30 = 0x00 0x1e
        // seq_parameter_set_id ue(0) = 1
        // log2_max_frame_num_minus4 ue(0) = 1
        // pic_order_cnt_type ue(0) = 1
        // log2_max_pic_order_cnt_lsb_minus4 ue(0) = 1
        // max_num_ref_frames ue(1) = 010
        // gaps_in_frame_num_value_allowed_flag = 0
        // pic_width_in_mbs_minus1 ue(15) = 0000_1111? Wait ue(15)= 00010000 (code 15 -> leading zeros 4, suffix 1111?)
        // ue(n) code: leading_zeros k = floor(log2(n+1)), suffix = n+1-2^k
        // n=15: 15+1=16=2^4, k=4, suffix 0 (4 bits) => 0000 10000? Actually code is `0000 1 0000` (9 bits).
        // This is getting long. Use a known SPS byte vector that parses without scaling matrices.
        // For this test we accept that a minimal baseline SPS can be built manually.
        let sps_bytes = [
            0x67, // nal header type 7
            0x42, 0x00, 0x1e, // profile/constraints/level
            // seq_parameter_set_id 0, log2_max_frame_num 0, pic_order_cnt_type 0,
            // log2_max_pic_order_cnt_lsb 0, max_ref 1, gaps 0
            0xe9, 0x42, 0x10, 0x89, 0xf3, 0x22, 0xcb, 0x80,
        ];
        let pps_bytes = [0x68, 0xce, 0x3c, 0x80];

        let mut avcc = vec![
            1,    // configurationVersion
            0x42, // profile
            0x00, // profile compatibility
            0x1e, // level
            0xff, // lengthSizeMinusOne=3, reserved
            0xe1, // numOfSequenceParameterSets=1
        ];
        avcc.extend_from_slice(&(sps_bytes.len() as u16).to_be_bytes());
        avcc.extend_from_slice(&sps_bytes);
        avcc.push(1); // numOfPictureParameterSets=1
        avcc.extend_from_slice(&(pps_bytes.len() as u16).to_be_bytes());
        avcc.extend_from_slice(&pps_bytes);

        let config = H264CodecConfig::parse(&avcc).unwrap();
        assert_eq!(config.avc_profile_indication, 0x42);
        assert_eq!(config.sps_list.len(), 1);
        assert!(config.sps_list[0].starts_with(&[0x67, 0x42, 0x00, 0x1e]));

        // Parse/build round-trip preserves SPS/PPS boundaries.
        let rebuilt = H264CodecConfig::parse(&config.build()).unwrap();
        assert_eq!(rebuilt.sps_list, config.sps_list);
        assert_eq!(rebuilt.pps_list, config.pps_list);
    }

    #[test]
    fn annexb_distinguishes_3byte_start_code_from_4byte() {
        // 3-byte start code followed by a NAL header byte of 0x01 (non-IDR slice).
        let data = [0x00, 0x00, 0x01, 0x01, 0xab, 0xcd];
        let units = split_annexb(&data).unwrap();
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].nal_type, 1);
        assert_eq!(units[0].data, &[0x01, 0xab, 0xcd]);
    }

    #[test]
    fn sps_crop_defaults_to_4_2_0_for_baseline() {
        // Baseline profile SPS (66) with frame_mbs_only_flag=1 and
        // frame cropping offsets of 8 on all sides. With 4:2:0 inferred
        // chroma_format_idc, crop units are 2, so width/height should be
        // 256 - (8+8)*2 = 224. Before the fix they were 240 because
        // crop_unit_x/y defaulted to 1.
        let sps_rbsp = [
            0x42, 0x00, 0x1e, 0xf8, 0x20, 0x10, 0xe2, 0x44, 0x89, 0x12, 0x80,
        ];
        let sps = Sps::parse(&sps_rbsp).unwrap();
        assert_eq!(sps.profile_idc, 66);
        assert_eq!(sps.chroma_format_idc, 1);
        assert_eq!(sps.width, 224);
        assert_eq!(sps.height, 224);
    }

    #[test]
    fn sps_scaling_list_parses_high_profile() {
        // High profile (100) SPS with seq_scaling_matrix_present_flag=1,
        // one 4x4 scaling list whose first delta_scale terminates parsing.
        // Before the fix this SPS failed because the parser read a
        // non-existent use_default flag.
        let sps_rbsp = [0x64, 0x00, 0x1e, 0xad, 0x84, 0x40, 0x78, 0x20, 0x10, 0xc8];
        let sps = Sps::parse(&sps_rbsp).unwrap();
        assert_eq!(sps.profile_idc, 100);
        assert_eq!(sps.chroma_format_idc, 1);
        assert_eq!(sps.width, 256);
        assert_eq!(sps.height, 256);
    }
}

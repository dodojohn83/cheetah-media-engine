//! H.265 (HEVC) bitstream helpers: NAL splitting, Annex-B/HVCC conversion,
//! IRAP classification, and minimal decoder configuration parsing.

extern crate alloc;

use alloc::vec::Vec;

pub mod parameter_sets;

pub use crate::rbsp::unescape_rbsp;
pub use parameter_sets::{ProfileTierLevel, Sps as H265Sps, Vps as H265Vps};

use crate::ByteCursor;

/// Errors specific to H.265 parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H265Error {
    TooShort,
    InvalidNalLength,
    InvalidStartCode,
    InvalidConfig,
    UnsupportedConfig,
}

impl core::fmt::Display for H265Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort => write!(f, "H.265 data too short"),
            Self::InvalidNalLength => write!(f, "H.265 invalid NAL length"),
            Self::InvalidStartCode => write!(f, "H.265 invalid Annex-B start code"),
            Self::InvalidConfig => write!(f, "H.265 invalid decoder configuration"),
            Self::UnsupportedConfig => write!(f, "H.265 unsupported decoder configuration"),
        }
    }
}

impl From<crate::ReadError> for H265Error {
    fn from(_: crate::ReadError) -> Self {
        Self::TooShort
    }
}

impl From<crate::bit::BitError> for H265Error {
    fn from(_: crate::bit::BitError) -> Self {
        Self::TooShort
    }
}

/// H.265 NAL unit type values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NalUnitType {
    TrailN = 0,
    TrailR = 1,
    TsaN = 2,
    TsaR = 3,
    StsaN = 4,
    StsaR = 5,
    RadlN = 6,
    RadlR = 7,
    RaslN = 8,
    RaslR = 9,
    BlaWLp = 16,
    BlaWRadl = 17,
    BlaNLp = 18,
    IdrWRadl = 19,
    IdrNLp = 20,
    CraNut = 21,
    RsvIrapVcl22 = 22,
    RsvIrapVcl23 = 23,
    VpsNut = 32,
    SpsNut = 33,
    PpsNut = 34,
    AudNut = 35,
    PrefixSeiNut = 39,
    SuffixSeiNut = 40,
    Unknown = 255,
}

impl NalUnitType {
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::TrailN,
            1 => Self::TrailR,
            2 => Self::TsaN,
            3 => Self::TsaR,
            4 => Self::StsaN,
            5 => Self::StsaR,
            6 => Self::RadlN,
            7 => Self::RadlR,
            8 => Self::RaslN,
            9 => Self::RaslR,
            16 => Self::BlaWLp,
            17 => Self::BlaWRadl,
            18 => Self::BlaNLp,
            19 => Self::IdrWRadl,
            20 => Self::IdrNLp,
            21 => Self::CraNut,
            22 => Self::RsvIrapVcl22,
            23 => Self::RsvIrapVcl23,
            32 => Self::VpsNut,
            33 => Self::SpsNut,
            34 => Self::PpsNut,
            35 => Self::AudNut,
            39 => Self::PrefixSeiNut,
            40 => Self::SuffixSeiNut,
            _ => Self::Unknown,
        }
    }

    /// True for all IRAP NAL types (16-23).
    pub const fn is_irap(self) -> bool {
        matches!(self as u8, 16..=23)
    }

    /// True for IDR slices.
    pub const fn is_idr(self) -> bool {
        matches!(self, Self::IdrWRadl | Self::IdrNLp)
    }

    /// True for CRA slices.
    pub const fn is_cra(self) -> bool {
        matches!(self, Self::CraNut)
    }

    /// True for BLA slices.
    pub const fn is_bla(self) -> bool {
        matches!(self, Self::BlaWLp | Self::BlaWRadl | Self::BlaNLp)
    }

    /// True for parameter-set NAL types.
    pub const fn is_parameter_set(self) -> bool {
        matches!(self, Self::VpsNut | Self::SpsNut | Self::PpsNut)
    }
}

/// A single H.265 NAL unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NalUnit<'a> {
    pub forbidden_zero_bit: u8,
    pub nal_unit_type: u8,
    pub nuh_layer_id: u8,
    pub nuh_temporal_id_plus1: u8,
    /// Raw NAL unit including the 2-byte header.
    pub data: &'a [u8],
    /// Payload after the 2-byte header.
    pub payload: &'a [u8],
}

impl<'a> NalUnit<'a> {
    pub fn nal_type(&self) -> NalUnitType {
        NalUnitType::from_u8(self.nal_unit_type)
    }

    pub fn is_irap(&self) -> bool {
        self.nal_type().is_irap()
    }

    pub fn is_idr(&self) -> bool {
        self.nal_type().is_idr()
    }

    /// Convert this NAL unit to Annex-B form with a 4-byte start code.
    pub fn to_annexb(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.data.len());
        out.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        out.extend_from_slice(self.data);
        out
    }

    /// Convert this NAL unit to HVCC form with `length_size` bytes.
    pub fn to_hvcc(&self, length_size: u8) -> Result<Vec<u8>, H265Error> {
        if !matches!(length_size, 1 | 2 | 4) {
            return Err(H265Error::InvalidNalLength);
        }
        let len = self.data.len();
        if (len as u64) >= (1u64 << (length_size * 8)) {
            return Err(H265Error::InvalidNalLength);
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
/// starting at `start`, skipping H.264/HEVC emulation prevention sequences.
///
/// Returns `(position, code_len)` where `code_len` is 3 or 4.
fn find_start_code(data: &[u8], start: usize) -> Option<(usize, usize)> {
    if data.len() < start.saturating_add(3) {
        return None;
    }
    let mut i = start;
    while i.saturating_add(2) < data.len() {
        if data[i] == 0x00 && data[i + 1] == 0x00 {
            if i + 3 < data.len() && data[i + 2] == 0x00 && data[i + 3] == 0x01 {
                return Some((i, 4));
            }
            if data[i + 2] == 0x01 {
                return Some((i, 3));
            }
            if data[i + 2] == 0x03 && i + 3 < data.len() && data[i + 3] <= 0x03 {
                // Emulation prevention: skip the entire 0x00 0x00 0x03 XX sequence
                // and resume scanning from the byte after the protected value.
                i += 4;
                continue;
            }
        }
        i += 1;
    }
    None
}

/// Split Annex-B H.265 data into NAL units.
pub fn split_annexb<'a>(data: &'a [u8]) -> Result<Vec<NalUnit<'a>>, H265Error> {
    let mut units = Vec::new();
    let mut pos = 0usize;

    while pos < data.len() {
        let (start, code_len) = find_start_code(data, pos).ok_or(H265Error::InvalidStartCode)?;
        let header_pos = start + code_len;
        if header_pos + 1 >= data.len() {
            break;
        }
        let h0 = data[header_pos];
        let h1 = data[header_pos + 1];
        let forbidden_zero_bit = (h0 >> 7) & 0x01;
        let nal_unit_type = (h0 >> 1) & 0x3f;
        let nuh_layer_id = ((h0 & 0x01) << 5) | ((h1 >> 3) & 0x1f);
        let nuh_temporal_id_plus1 = h1 & 0x07;

        let (next, _) = find_start_code(data, header_pos + 2).unwrap_or((data.len(), 3));
        let unit_data = &data[header_pos..next];
        let payload = if unit_data.len() >= 2 {
            &unit_data[2..]
        } else {
            &[]
        };

        units.push(NalUnit {
            forbidden_zero_bit,
            nal_unit_type,
            nuh_layer_id,
            nuh_temporal_id_plus1,
            data: unit_data,
            payload,
        });
        pos = next;
    }

    Ok(units)
}

/// Parse HVCC length-prefixed NAL units.
pub fn split_hvcc<'a>(data: &'a [u8], length_size: u8) -> Result<Vec<NalUnit<'a>>, H265Error> {
    if !matches!(length_size, 1 | 2 | 4) {
        return Err(H265Error::InvalidNalLength);
    }
    let mut units = Vec::new();
    let mut cursor = ByteCursor::new(data);

    while !cursor.is_empty() {
        let len = match length_size {
            1 => cursor.read_u8()? as usize,
            2 => cursor.read_u16_be()? as usize,
            4 => cursor.read_u32_be()? as usize,
            _ => unreachable!(),
        };
        if len > cursor.remaining() {
            return Err(H265Error::InvalidNalLength);
        }
        let bytes = cursor.read_bytes(len)?;
        if bytes.len() < 2 {
            continue;
        }
        let h0 = bytes[0];
        let h1 = bytes[1];
        let forbidden_zero_bit = (h0 >> 7) & 0x01;
        let nal_unit_type = (h0 >> 1) & 0x3f;
        let nuh_layer_id = ((h0 & 0x01) << 5) | ((h1 >> 3) & 0x1f);
        let nuh_temporal_id_plus1 = h1 & 0x07;
        units.push(NalUnit {
            forbidden_zero_bit,
            nal_unit_type,
            nuh_layer_id,
            nuh_temporal_id_plus1,
            data: bytes,
            payload: &bytes[2..],
        });
    }

    Ok(units)
}

/// Convert Annex-B data to HVCC length-prefixed data.
pub fn annexb_to_hvcc(data: &[u8], length_size: u8) -> Result<Vec<u8>, H265Error> {
    let units = split_annexb(data)?;
    let mut out = Vec::new();
    for unit in units {
        out.extend_from_slice(&unit.to_hvcc(length_size)?);
    }
    Ok(out)
}

/// Convert HVCC length-prefixed data to Annex-B data.
pub fn hvcc_to_annexb(data: &[u8], length_size: u8) -> Result<Vec<u8>, H265Error> {
    let units = split_hvcc(data, length_size)?;
    let mut out = Vec::new();
    for unit in units {
        out.extend_from_slice(&unit.to_annexb());
    }
    Ok(out)
}

/// Minimal H.265 decoder configuration (HEVCDecoderConfigurationRecord).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct H265CodecConfig {
    pub configuration_version: u8,
    pub general_profile_space: u8,
    pub general_tier_flag: u8,
    pub general_profile_idc: u8,
    pub general_profile_compatibility_flags: u32,
    pub general_constraint_indicator_flags: u64,
    pub general_level_idc: u8,
    pub min_spatial_segmentation_idc: u16,
    pub parallelism_type: u8,
    pub chroma_format: u8,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
    pub avg_frame_rate: u16,
    pub constant_frame_rate: u8,
    pub num_temporal_layers: u8,
    pub temporal_id_nested: bool,
    pub length_size_minus_one: u8,
    /// Each VPS NAL unit, including its 2-byte header.
    pub vps_list: Vec<Vec<u8>>,
    /// Each SPS NAL unit, including its 2-byte header.
    pub sps_list: Vec<Vec<u8>>,
    /// Each PPS NAL unit, including its 2-byte header.
    pub pps_list: Vec<Vec<u8>>,
    pub codec_string: alloc::string::String,
}

impl H265CodecConfig {
    /// Parse an HEVCDecoderConfigurationRecord.
    #[allow(clippy::field_reassign_with_default)]
    pub fn parse(data: &[u8]) -> Result<Self, H265Error> {
        if data.len() < 23 {
            return Err(H265Error::TooShort);
        }
        let mut cursor = ByteCursor::new(data);
        let mut cfg = Self::default();
        cfg.configuration_version = cursor.read_u8()?;
        if cfg.configuration_version != 1 {
            return Err(H265Error::InvalidConfig);
        }
        let b = cursor.read_u8()?;
        cfg.general_profile_space = (b >> 6) & 0x03;
        cfg.general_tier_flag = (b >> 5) & 0x01;
        cfg.general_profile_idc = b & 0x1f;
        cfg.general_profile_compatibility_flags = cursor.read_u32_be()?;

        let mut constraint = 0u64;
        for _ in 0..6 {
            constraint = (constraint << 8) | cursor.read_u8()? as u64;
        }
        cfg.general_constraint_indicator_flags = constraint;
        cfg.general_level_idc = cursor.read_u8()?;

        let b1 = cursor.read_u16_be()?;
        cfg.min_spatial_segmentation_idc = b1 & 0x0fff;

        let b2 = cursor.read_u8()?;
        cfg.parallelism_type = b2 & 0x03;

        let b3 = cursor.read_u8()?;
        cfg.chroma_format = b3 & 0x03;

        let b4 = cursor.read_u8()?;
        cfg.bit_depth_luma_minus8 = b4 & 0x07;

        let b5 = cursor.read_u8()?;
        cfg.bit_depth_chroma_minus8 = b5 & 0x07;

        cfg.avg_frame_rate = cursor.read_u16_be()?;

        let b6 = cursor.read_u8()?;
        cfg.constant_frame_rate = (b6 >> 6) & 0x03;
        cfg.num_temporal_layers = (b6 >> 3) & 0x07;
        cfg.temporal_id_nested = ((b6 >> 2) & 0x01) != 0;
        cfg.length_size_minus_one = b6 & 0x03;

        let num_arrays = cursor.read_u8()?;
        for _ in 0..num_arrays {
            let arr_header = cursor.read_u8()?;
            let nal_type = arr_header & 0x3f;
            let num_nalus = cursor.read_u16_be()?;
            for _ in 0..num_nalus {
                let len = cursor.read_u16_be()? as usize;
                if len > cursor.remaining() {
                    return Err(H265Error::InvalidNalLength);
                }
                let nal = cursor.read_bytes(len)?;
                let target = match nal_type {
                    32 => &mut cfg.vps_list,
                    33 => &mut cfg.sps_list,
                    34 => &mut cfg.pps_list,
                    _ => continue,
                };
                target.push(nal.to_vec());
            }
        }

        cfg.codec_string = Self::build_codec_string(
            cfg.general_profile_space,
            cfg.general_tier_flag,
            cfg.general_profile_idc,
            cfg.general_profile_compatibility_flags,
            cfg.general_constraint_indicator_flags,
            cfg.general_level_idc,
        );

        Ok(cfg)
    }

    pub fn build_codec_string(
        profile_space: u8,
        tier: u8,
        profile_idc: u8,
        profile_compatibility_flags: u32,
        constraint: u64,
        level_idc: u8,
    ) -> alloc::string::String {
        // RFC 6381 / ISO 14496-15 E.3 HEVC codec string:
        //   hev1.<profile_space_letter?><profile_idc>.<profile_compatibility_hex>
        //       .<tier_char><level_idc>.[<constraint_bytes...>]
        // profile_space letters: 0 omitted, 1=A, 2=B, 3=C. Trailing zero
        // constraint bytes are omitted.
        let tier_char = if tier == 0 { 'L' } else { 'H' };
        let profile_prefix = match profile_space {
            0 => "",
            1 => "A",
            2 => "B",
            3 => "C",
            _ => "",
        };
        let constraint = constraint & 0xffff_ffff_ffff; // keep 48 bits
        let mut bytes = [0u8; 6];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = ((constraint >> ((5 - i) * 8)) & 0xff) as u8;
        }
        let last = bytes
            .iter()
            .rposition(|&b| b != 0)
            .map(|i| i + 1)
            .unwrap_or(0);

        let mut out = alloc::format!(
            "hev1.{}{}.{:x}.{}{}",
            profile_prefix,
            profile_idc,
            profile_compatibility_flags.reverse_bits(),
            tier_char,
            level_idc
        );
        if last > 0 {
            out.push('.');
            for (i, byte) in bytes.iter().take(last).enumerate() {
                if i > 0 {
                    out.push('.');
                }
                out.push_str(&alloc::format!("{:02x}", byte));
            }
        }
        out
    }

    /// Build a minimal HEVCDecoderConfigurationRecord containing VPS/SPS/PPS arrays.
    pub fn build(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(self.configuration_version);
        let b = ((self.general_profile_space & 0x03) << 6)
            | ((self.general_tier_flag & 0x01) << 5)
            | (self.general_profile_idc & 0x1f);
        out.push(b);
        out.extend_from_slice(&self.general_profile_compatibility_flags.to_be_bytes());
        for i in (0..6).rev() {
            out.push(((self.general_constraint_indicator_flags >> (i * 8)) & 0xff) as u8);
        }
        out.push(self.general_level_idc);
        out.extend_from_slice(
            &(0xf000 | (self.min_spatial_segmentation_idc & 0x0fff)).to_be_bytes(),
        );
        out.push(0xfc | (self.parallelism_type & 0x03));
        out.push(0xfc | (self.chroma_format & 0x03));
        out.push(0xf8 | (self.bit_depth_luma_minus8 & 0x07));
        out.push(0xf8 | (self.bit_depth_chroma_minus8 & 0x07));
        out.extend_from_slice(&self.avg_frame_rate.to_be_bytes());
        let b = ((self.constant_frame_rate & 0x03) << 6)
            | ((self.num_temporal_layers & 0x07) << 3)
            | ((if self.temporal_id_nested { 1 } else { 0 }) << 2)
            | (self.length_size_minus_one & 0x03);
        out.push(b);

        let mut nal_arrays: Vec<(u8, &[Vec<u8>])> = Vec::new();
        if !self.vps_list.is_empty() {
            nal_arrays.push((32u8, &self.vps_list));
        }
        if !self.sps_list.is_empty() {
            nal_arrays.push((33u8, &self.sps_list));
        }
        if !self.pps_list.is_empty() {
            nal_arrays.push((34u8, &self.pps_list));
        }

        out.push(nal_arrays.len() as u8);
        for (nal_type, nals) in nal_arrays {
            out.push(nal_type & 0x3f);
            out.extend_from_slice(&(nals.len() as u16).to_be_bytes());
            for nal in nals {
                out.extend_from_slice(&(nal.len() as u16).to_be_bytes());
                out.extend_from_slice(nal);
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h265_annexb_round_trip() {
        // VPS (type 32), SPS (33), PPS (34), IDR (20) with minimal 2-byte headers.
        let mut annexb = Vec::new();
        let vps = [0x40, 0x01]; // header for VPS
        let sps = [0x42, 0x01]; // header for SPS
        let pps = [0x44, 0x01]; // header for PPS
        let idr = [0x28, 0x01]; // header for IDR_N_LP (20)
        for nal in [&vps[..], &sps[..], &pps[..], &idr[..]] {
            annexb.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
            annexb.extend_from_slice(nal);
        }

        let hvcc = annexb_to_hvcc(&annexb, 4).unwrap();
        let units = split_hvcc(&hvcc, 4).unwrap();
        assert_eq!(units.len(), 4);
        assert_eq!(units[0].nal_unit_type, 32);
        assert_eq!(units[1].nal_unit_type, 33);
        assert_eq!(units[2].nal_unit_type, 34);
        assert_eq!(units[3].nal_unit_type, 20);
        assert!(units[3].is_idr());

        let back = hvcc_to_annexb(&hvcc, 4).unwrap();
        let expected = [
            0x00, 0x00, 0x00, 0x01, 0x40, 0x01, 0x00, 0x00, 0x00, 0x01, 0x42, 0x01, 0x00, 0x00,
            0x00, 0x01, 0x44, 0x01, 0x00, 0x00, 0x00, 0x01, 0x28, 0x01,
        ];
        assert_eq!(back, expected);
    }

    #[test]
    fn h265_codec_config_round_trip() {
        let vps = [0x40u8, 0x01];
        let sps = [0x42u8, 0x01];
        let pps = [0x44u8, 0x01];
        let cfg = H265CodecConfig {
            configuration_version: 1,
            general_profile_space: 0,
            general_tier_flag: 0,
            general_profile_idc: 1,
            general_profile_compatibility_flags: 0x60000000,
            general_constraint_indicator_flags: 0,
            general_level_idc: 93,
            min_spatial_segmentation_idc: 0,
            parallelism_type: 0,
            chroma_format: 1,
            bit_depth_luma_minus8: 0,
            bit_depth_chroma_minus8: 0,
            avg_frame_rate: 0,
            constant_frame_rate: 0,
            num_temporal_layers: 1,
            temporal_id_nested: true,
            length_size_minus_one: 3,
            vps_list: vec![vps.to_vec()],
            sps_list: vec![sps.to_vec()],
            pps_list: vec![pps.to_vec()],
            codec_string: String::new(),
        };
        let bytes = cfg.build();
        let parsed = H265CodecConfig::parse(&bytes).unwrap();
        assert_eq!(parsed.vps_list, vec![vps.to_vec()]);
        assert_eq!(parsed.sps_list, vec![sps.to_vec()]);
        assert_eq!(parsed.pps_list, vec![pps.to_vec()]);
        assert_eq!(parsed.general_profile_idc, 1);
        assert_eq!(parsed.general_level_idc, 93);
        assert_eq!(parsed.codec_string, "hev1.1.6.L93");

        // Parse/build round-trip preserves VPS/SPS/PPS boundaries.
        let rebuilt = H265CodecConfig::parse(&parsed.build()).unwrap();
        assert_eq!(rebuilt.vps_list, parsed.vps_list);
        assert_eq!(rebuilt.sps_list, parsed.sps_list);
        assert_eq!(rebuilt.pps_list, parsed.pps_list);
        assert_eq!(rebuilt.codec_string, parsed.codec_string);
    }

    #[test]
    fn annexb_skips_emulation_prevention_3byte() {
        // NAL1 payload contains an emulation-prevention sequence 00 00 03 01,
        // which must not be mistaken for a 3-byte start code.
        let data = [
            0x00, 0x00, 0x00, 0x01, // start code
            0x40, 0x01, // H.265 NAL header: nal_type=32 (VPS), layer=0, temporal=1
            0x0a, 0x00, 0x00, 0x03, 0x01, 0xab, // payload with EPB
            0x00, 0x00, 0x00, 0x01, // next start code
            0x26, 0x01, // NAL header: nal_type=19 (IDR_W_RADL)
            0x88,
        ];
        let units = split_annexb(&data).unwrap();
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].nal_unit_type, 32);
        assert_eq!(units[1].nal_unit_type, 19);
        assert!(units[0].data.len() > 2);
    }

    #[test]
    fn annexb_distinguishes_3byte_start_code_from_4byte() {
        // 3-byte start code followed by a NAL header whose first byte is 0x01.
        let data = [0x00, 0x00, 0x01, 0x01, 0x02, 0xab, 0xcd];
        let units = split_annexb(&data).unwrap();
        assert_eq!(units.len(), 1);
        // nal_unit_type is (0x01 >> 1) & 0x3f = 0.
        assert_eq!(units[0].nal_unit_type, 0);
        assert_eq!(units[0].data, &[0x01, 0x02, 0xab, 0xcd]);
    }

    #[test]
    fn reserved_irap_nal_types_recognized() {
        // Reserved IRAP NAL types 22 and 23 must still be detected as IRAP.
        for nal_type in [22u8, 23u8] {
            let header0 = (nal_type << 1) & 0x7e; // forbidden=0, nal_type in upper 6 bits
            let header1 = 0x01; // nuh_layer_id=0, temporal_id_plus1=1
            let data = [0x00, 0x00, 0x00, 0x01, header0, header1, 0xab, 0xcd];
            let units = split_annexb(&data).unwrap();
            assert_eq!(units.len(), 1);
            assert_eq!(units[0].nal_unit_type, nal_type);
            assert!(units[0].is_irap());
        }
    }
}

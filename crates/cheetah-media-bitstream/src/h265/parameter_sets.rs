//! Minimal H.265 parameter set parsing for decoder configuration.

extern crate alloc;

use alloc::vec::Vec;

use super::{H265Error, NalUnitType};
use crate::bit::BitCursor;
use crate::rbsp::unescape_rbsp;

/// Profile, tier and level extracted from VPS or SPS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProfileTierLevel {
    pub general_profile_space: u8,
    pub general_tier_flag: u8,
    pub general_profile_idc: u8,
    pub general_profile_compatibility_flags: u32,
    pub general_constraint_indicator_flags: u64,
    pub general_level_idc: u8,
}

impl ProfileTierLevel {
    /// Parse a `profile_tier_level` structure.
    ///
    /// `profile_tier_present_flag` is true for the general PTL in VPS/SPS and
    /// false for sub-layer PTLs; `max_num_sub_layers_minus1` controls how
    /// many sub-layer presence flags are read.
    pub fn parse(
        cursor: &mut BitCursor,
        profile_tier_present_flag: bool,
        max_num_sub_layers_minus1: u8,
    ) -> Result<Self, H265Error> {
        let mut ptl = Self::default();

        if profile_tier_present_flag {
            ptl.general_profile_space = cursor.read_bits(2)? as u8;
            ptl.general_tier_flag = cursor.read_bits(1)? as u8;
            ptl.general_profile_idc = cursor.read_bits(5)? as u8;
            ptl.general_profile_compatibility_flags = cursor.read_u32(32)?;
            // The 48-bit constraint indicator includes four source/constraint
            // flags in its most-significant bits followed by 44 additional bits.
            ptl.general_constraint_indicator_flags = cursor.read_bits(48)?;
        }

        ptl.general_level_idc = cursor.read_u32(8)? as u8;

        let mut sub_layer_profile_present = Vec::new();
        let mut sub_layer_level_present = Vec::new();
        for _ in 0..max_num_sub_layers_minus1 {
            sub_layer_profile_present.push(cursor.read_bool()?);
            sub_layer_level_present.push(cursor.read_bool()?);
        }

        // H.265 section 7.3.3: reserved_zero_2bits padding for the remaining
        // sub-layer indices up to 8 when temporal scalability is present.
        if max_num_sub_layers_minus1 > 0 {
            for _ in max_num_sub_layers_minus1..8 {
                cursor.read_bits(2)?;
            }
        }

        for i in 0..max_num_sub_layers_minus1 as usize {
            if sub_layer_profile_present[i] {
                // sub-layer profile_space(2), tier_flag(1), profile_idc(5),
                // profile_compatibility_flags(32), 48-bit constraint indicator.
                cursor.read_bits(8)?;
                cursor.read_u32(32)?;
                cursor.read_bits(48)?;
            }
            if sub_layer_level_present[i] {
                cursor.read_u32(8)?;
            }
        }

        Ok(ptl)
    }
}

/// Minimal H.265 Video Parameter Set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vps {
    pub profile_tier_level: ProfileTierLevel,
    pub max_sub_layers_minus1: u8,
}

impl Vps {
    /// Parse a VPS NAL unit (header + RBSP with emulation prevention bytes).
    pub fn parse(nal: &[u8]) -> Result<Self, H265Error> {
        if nal.len() < 2 {
            return Err(H265Error::TooShort);
        }
        let rbsp = unescape_rbsp(&nal[2..]);
        let mut cursor = BitCursor::new(&rbsp);

        // vps_video_parameter_set_id: u(4)
        cursor.read_bits(4)?;
        // vps_base_layer_internal_flag: u(1)
        cursor.read_bool()?;
        // vps_base_layer_available_flag: u(1)
        cursor.read_bool()?;
        // vps_max_layers_minus1: u(6)
        cursor.read_bits(6)?;
        let max_sub_layers_minus1 = cursor.read_bits(3)? as u8;
        // vps_temporal_id_nesting_flag: u(1)
        cursor.read_bool()?;
        // vps_reserved_0xffff_16bits: u(16)
        cursor.read_u32(16)?;

        let ptl = ProfileTierLevel::parse(&mut cursor, true, max_sub_layers_minus1)?;

        // vps_sub_layer_ordering_info_present_flag: u(1)
        let sub_layer_ordering_info_present = cursor.read_bool()?;
        let start = if sub_layer_ordering_info_present {
            0
        } else {
            max_sub_layers_minus1
        };
        for _ in start..=max_sub_layers_minus1 {
            cursor.read_ue()?; // vps_max_dec_pic_buffering_minus1
            cursor.read_ue()?; // vps_max_num_reorder_pics
            cursor.read_ue()?; // vps_max_latency_increase_plus1
        }

        Ok(Self {
            profile_tier_level: ptl,
            max_sub_layers_minus1,
        })
    }
}

/// Minimal H.265 Sequence Parameter Set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sps {
    pub profile_tier_level: ProfileTierLevel,
    pub max_sub_layers_minus1: u8,
    pub temporal_id_nesting_flag: bool,
    pub chroma_format_idc: u8,
    pub separate_colour_plane_flag: bool,
    pub pic_width_in_luma_samples: u32,
    pub pic_height_in_luma_samples: u32,
    pub conf_win_left_offset: u32,
    pub conf_win_right_offset: u32,
    pub conf_win_top_offset: u32,
    pub conf_win_bottom_offset: u32,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
}

impl Sps {
    /// Visible width after applying conformance window cropping.
    pub fn width(self) -> u32 {
        let (sub_width, _) =
            Self::chroma_subsampling(self.chroma_format_idc, self.separate_colour_plane_flag);
        let crop_width = self
            .conf_win_left_offset
            .saturating_add(self.conf_win_right_offset)
            .saturating_mul(sub_width);
        self.pic_width_in_luma_samples
            .saturating_sub(crop_width)
            .max(1)
    }

    /// Visible height after applying conformance window cropping.
    pub fn height(self) -> u32 {
        let (_, sub_height) =
            Self::chroma_subsampling(self.chroma_format_idc, self.separate_colour_plane_flag);
        let crop_height = self
            .conf_win_top_offset
            .saturating_add(self.conf_win_bottom_offset)
            .saturating_mul(sub_height);
        self.pic_height_in_luma_samples
            .saturating_sub(crop_height)
            .max(1)
    }

    fn chroma_subsampling(chroma_format_idc: u8, separate_colour_plane_flag: bool) -> (u32, u32) {
        match chroma_format_idc {
            0 => (1, 1), // monochrome
            1 => (2, 2), // 4:2:0
            2 => (2, 1), // 4:2:2
            3 if separate_colour_plane_flag => (1, 1),
            3 => (1, 1), // 4:4:4
            _ => (1, 1),
        }
    }

    /// Parse an SPS NAL unit (header + RBSP with emulation prevention bytes).
    pub fn parse(nal: &[u8]) -> Result<Self, H265Error> {
        if nal.len() < 2 {
            return Err(H265Error::TooShort);
        }
        let nal_type = (nal[0] >> 1) & 0x3f;
        if nal_type != NalUnitType::SpsNut as u8 {
            return Err(H265Error::InvalidConfig);
        }

        let rbsp = unescape_rbsp(&nal[2..]);
        let mut cursor = BitCursor::new(&rbsp);

        // sps_video_parameter_set_id: u(4)
        cursor.read_bits(4)?;
        let max_sub_layers_minus1 = cursor.read_bits(3)? as u8;
        let temporal_id_nesting_flag = cursor.read_bool()?;
        let ptl = ProfileTierLevel::parse(&mut cursor, true, max_sub_layers_minus1)?;

        // sps_seq_parameter_set_id: ue(v)
        cursor.read_ue()?;
        let chroma_format_idc =
            u8::try_from(cursor.read_ue()?).map_err(|_| H265Error::InvalidConfig)?;
        if chroma_format_idc > 3 {
            return Err(H265Error::InvalidConfig);
        }
        let separate_colour_plane_flag = if chroma_format_idc == 3 {
            cursor.read_bool()?
        } else {
            false
        };

        let pic_width_in_luma_samples =
            u32::try_from(cursor.read_ue()?).map_err(|_| H265Error::InvalidConfig)?;
        let pic_height_in_luma_samples =
            u32::try_from(cursor.read_ue()?).map_err(|_| H265Error::InvalidConfig)?;

        let conformance_window_flag = cursor.read_bool()?;
        let mut conf_win_left_offset = 0u32;
        let mut conf_win_right_offset = 0u32;
        let mut conf_win_top_offset = 0u32;
        let mut conf_win_bottom_offset = 0u32;
        if conformance_window_flag {
            conf_win_left_offset =
                u32::try_from(cursor.read_ue()?).map_err(|_| H265Error::InvalidConfig)?;
            conf_win_right_offset =
                u32::try_from(cursor.read_ue()?).map_err(|_| H265Error::InvalidConfig)?;
            conf_win_top_offset =
                u32::try_from(cursor.read_ue()?).map_err(|_| H265Error::InvalidConfig)?;
            conf_win_bottom_offset =
                u32::try_from(cursor.read_ue()?).map_err(|_| H265Error::InvalidConfig)?;
        }

        let bit_depth_luma_minus8 =
            u8::try_from(cursor.read_ue()?).map_err(|_| H265Error::InvalidConfig)?;
        let bit_depth_chroma_minus8 =
            u8::try_from(cursor.read_ue()?).map_err(|_| H265Error::InvalidConfig)?;

        Ok(Self {
            profile_tier_level: ptl,
            max_sub_layers_minus1,
            temporal_id_nesting_flag,
            chroma_format_idc,
            separate_colour_plane_flag,
            pic_width_in_luma_samples,
            pic_height_in_luma_samples,
            conf_win_left_offset,
            conf_win_right_offset,
            conf_win_top_offset,
            conf_win_bottom_offset,
            bit_depth_luma_minus8,
            bit_depth_chroma_minus8,
        })
    }
}

impl Default for Sps {
    fn default() -> Self {
        Self {
            profile_tier_level: ProfileTierLevel::default(),
            max_sub_layers_minus1: 0,
            temporal_id_nesting_flag: false,
            chroma_format_idc: 1,
            separate_colour_plane_flag: false,
            pic_width_in_luma_samples: 0,
            pic_height_in_luma_samples: 0,
            conf_win_left_offset: 0,
            conf_win_right_offset: 0,
            conf_win_top_offset: 0,
            conf_win_bottom_offset: 0,
            bit_depth_luma_minus8: 0,
            bit_depth_chroma_minus8: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal parameter sets generated by FFmpeg/libx265 for a 64x64 progressive
    // 4:2:0 8-bit HEVC stream.
    const VPS: &[u8] = &[
        0x40, 0x01, 0x0c, 0x01, 0xff, 0xff, 0x04, 0x08, 0x00, 0x00, 0x03, 0x00, 0x9e, 0x08, 0x00,
        0x00, 0x03, 0x00, 0x00, 0x1e, 0x95, 0x94, 0x09,
    ];
    const SPS: &[u8] = &[
        0x42, 0x01, 0x01, 0x04, 0x08, 0x00, 0x00, 0x03, 0x00, 0x9e, 0x08, 0x00, 0x00, 0x03, 0x00,
        0x00, 0x1e, 0x90, 0x04, 0x10, 0x20, 0xb2, 0xca, 0xca, 0x94, 0x98, 0x5e, 0x02, 0xdc, 0x08,
        0x08, 0x00, 0x10, 0x00, 0x00, 0x03, 0x00, 0x10, 0x00, 0x00, 0x03, 0x01, 0xe0, 0x80,
    ];
    #[test]
    fn parses_x265_vps() {
        let vps = Vps::parse(VPS).unwrap();
        assert_eq!(vps.max_sub_layers_minus1, 0);
        assert_eq!(vps.profile_tier_level.general_profile_idc, 4); // Rext
        assert_eq!(vps.profile_tier_level.general_level_idc, 30); // 1.0
    }

    #[test]
    fn parses_x265_sps_dimensions() {
        let sps = Sps::parse(SPS).unwrap();
        assert_eq!(sps.chroma_format_idc, 3);
        assert!(!sps.separate_colour_plane_flag);
        assert_eq!(sps.pic_width_in_luma_samples, 64);
        assert_eq!(sps.pic_height_in_luma_samples, 64);
        assert_eq!(sps.width(), 64);
        assert_eq!(sps.height(), 64);
        assert_eq!(sps.bit_depth_luma_minus8, 0);
        assert_eq!(sps.bit_depth_chroma_minus8, 0);
    }

    #[test]
    fn parses_x265_sps_profile() {
        let sps = Sps::parse(SPS).unwrap();
        assert_eq!(sps.profile_tier_level.general_profile_idc, 4); // Rext
        assert_eq!(sps.profile_tier_level.general_level_idc, 30); // 1.0
    }

    #[test]
    fn rejects_short_nal() {
        assert!(Vps::parse(&[0x40]).is_err());
        assert!(Sps::parse(&[0x42, 0x01]).is_err());
    }

    #[test]
    fn parses_profile_tier_level_with_temporal_layers() {
        // 14 bytes = 112 bits: general PTL (96 bits) + 1 sub-layer presence
        // flags (2 bits) + reserved_zero_2bits padding for indices 1..7 (14 bits).
        let data = [
            0x01, // profile_space=0, tier=0, profile_idc=1
            0x00, 0x00, 0x00, 0x00, // profile_compatibility_flags = 0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 48-bit constraint indicator = 0
            0x5d, // general_level_idc = 93
            0x00, // sub-layer flags and reserved padding
            0x00, // reserved padding
        ];
        let mut cursor = crate::bit::BitCursor::new(&data);
        let ptl = ProfileTierLevel::parse(&mut cursor, true, 1).unwrap();
        assert_eq!(ptl.general_profile_idc, 1);
        assert_eq!(ptl.general_level_idc, 0x5d);
    }
}

//! Parameter set caches for H.264 and H.265 Annex-B streams.

use alloc::vec;
use alloc::vec::Vec;
use cheetah_media_bitstream::h264::{
    H264CodecConfig, Sps as H264Sps, unescape_rbsp as h264_unescape,
};
use cheetah_media_bitstream::h265::{H265CodecConfig, H265Sps, H265Vps};
use cheetah_media_types::{CodecConfig, CodecId, ColorSpace, PixelFormat, TrackInfo, VideoFormat};

/// Unified parameter-set cache for an Annex-B video track.
#[derive(Debug, Clone)]
pub enum ParameterSetCache {
    H264(H264ParameterSetCache),
    H265(H265ParameterSetCache),
}

impl ParameterSetCache {
    /// Create an empty cache for `codec`.
    pub fn new(codec: CodecId) -> Self {
        match codec {
            CodecId::H265 => Self::H265(H265ParameterSetCache::new()),
            _ => Self::H264(H264ParameterSetCache::new()),
        }
    }

    /// Reset the cache and any derived track state.
    pub fn reset(&mut self) {
        match self {
            Self::H264(c) => c.reset(),
            Self::H265(c) => c.reset(),
        }
    }

    /// True when enough parameter sets have been cached to build a decoder
    /// configuration record.
    pub fn is_complete(&self) -> bool {
        match self {
            Self::H264(c) => c.is_complete(),
            Self::H265(c) => c.is_complete(),
        }
    }

    /// If `nal` is a parameter-set NAL for this codec, consume it and return
    /// true. The caller should then call `update_track` and emit a `Track`
    /// event if the configuration changed.
    pub fn consume(&mut self, nal: &[u8]) -> bool {
        match self {
            Self::H264(c) => c.consume(nal),
            Self::H265(c) => c.consume(nal),
        }
    }

    /// Apply the current configuration and video format to `track`.
    pub fn update_track(&self, track: &mut TrackInfo) {
        match self {
            Self::H264(c) => c.update_track(track),
            Self::H265(c) => c.update_track(track),
        }
    }
}

/// Caches the latest H.264 SPS/PPS NAL units and derives an `AvcC` config.
#[derive(Debug, Clone, Default)]
pub struct H264ParameterSetCache {
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
}

impl H264ParameterSetCache {
    /// Create an empty cache.
    pub const fn new() -> Self {
        Self {
            sps: None,
            pps: None,
        }
    }

    /// True when both SPS and PPS have been observed.
    pub fn is_complete(&self) -> bool {
        self.sps.is_some() && self.pps.is_some()
    }

    /// Reset the cache.
    pub fn reset(&mut self) {
        self.sps = None;
        self.pps = None;
    }

    /// If `nal` is an SPS or PPS, store it and return true.
    pub fn consume(&mut self, nal: &[u8]) -> bool {
        if nal.is_empty() {
            return false;
        }
        let nal_type = nal[0] & 0x1f;
        match nal_type {
            7 => {
                self.sps = Some(nal.to_vec());
                true
            }
            8 => {
                self.pps = Some(nal.to_vec());
                true
            }
            _ => false,
        }
    }

    /// Build a `CodecConfig::AvcC` from the cached parameter sets, if complete.
    pub fn build_config(&self) -> Option<CodecConfig> {
        let (sps_nal, pps_nal) = (self.sps.as_ref()?, self.pps.as_ref()?);
        if sps_nal.is_empty() || pps_nal.is_empty() {
            return None;
        }
        let sps_rbsp = h264_unescape(&sps_nal[1..]);
        let parsed = H264Sps::parse(&sps_rbsp).ok()?;

        // AvcC records store the raw NAL bytes (header + RBSP with EPB intact);
        // H264CodecConfig::parse will unescape them when reading.
        let sps_for_config = sps_nal.to_vec();
        let pps_for_config = pps_nal.to_vec();

        let cfg = H264CodecConfig {
            configuration_version: 1,
            avc_profile_indication: parsed.profile_idc,
            profile_compatibility: parsed.constraint_set_flags,
            avc_level_indication: parsed.level_idc,
            length_size_minus_one: 3,
            sps_list: vec![sps_for_config],
            pps_list: vec![pps_for_config],
            width: parsed.width,
            height: parsed.height,
            codec_string: parsed.codec_string(),
        };

        Some(CodecConfig::AvcC(cfg.build().ok()?))
    }

    /// Build a `VideoFormat` from the cached SPS, if available and valid.
    pub fn video_format(&self) -> Option<VideoFormat> {
        let sps_nal = self.sps.as_ref()?;
        if sps_nal.len() < 2 {
            return None;
        }
        let sps_rbsp = h264_unescape(&sps_nal[1..]);
        let parsed = H264Sps::parse(&sps_rbsp).ok()?;
        Some(VideoFormat {
            pixel_format: PixelFormat::Yuv420P,
            coded_width: parsed.width,
            coded_height: parsed.height,
            visible_width: parsed.width,
            visible_height: parsed.height,
            stride: parsed.width,
            color_space: ColorSpace::Unspecified,
        })
    }

    /// Update a `TrackInfo` with the current AvcC config and video format.
    pub fn update_track(&self, track: &mut TrackInfo) {
        if let Some(config) = self.build_config() {
            track.set_codec_config(config);
        }
        if let Some(format) = self.video_format() {
            let _ = track.set_video_format(format);
        }
    }
}

/// Caches the latest H.265 VPS/SPS/PPS NAL units and derives an `HevcC` config.
#[derive(Debug, Clone, Default)]
pub struct H265ParameterSetCache {
    vps: Option<Vec<u8>>,
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
}

impl H265ParameterSetCache {
    /// Create an empty cache.
    pub const fn new() -> Self {
        Self {
            vps: None,
            sps: None,
            pps: None,
        }
    }

    /// True when SPS and PPS have been observed. VPS is optional but preferred
    /// for profile/tier/level information.
    pub fn is_complete(&self) -> bool {
        self.sps.is_some() && self.pps.is_some()
    }

    /// Reset the cache.
    pub fn reset(&mut self) {
        self.vps = None;
        self.sps = None;
        self.pps = None;
    }

    /// If `nal` is a VPS, SPS or PPS, store it and return true.
    pub fn consume(&mut self, nal: &[u8]) -> bool {
        if nal.len() < 2 {
            return false;
        }
        let nal_type = (nal[0] >> 1) & 0x3f;
        match nal_type {
            32 => {
                self.vps = Some(nal.to_vec());
                true
            }
            33 => {
                self.sps = Some(nal.to_vec());
                true
            }
            34 => {
                self.pps = Some(nal.to_vec());
                true
            }
            _ => false,
        }
    }

    fn pixel_format(chroma_format_idc: u8, separate_colour_plane_flag: bool) -> PixelFormat {
        if chroma_format_idc == 3 && separate_colour_plane_flag {
            return PixelFormat::Yuv444P;
        }
        match chroma_format_idc {
            0 => PixelFormat::Unknown(0),
            1 => PixelFormat::Yuv420P,
            2 => PixelFormat::Yuv422P,
            3 => PixelFormat::Yuv444P,
            _ => PixelFormat::Unknown(chroma_format_idc as u32),
        }
    }

    /// Build a `CodecConfig::HevcC` from the cached parameter sets, if complete.
    pub fn build_config(&self) -> Option<CodecConfig> {
        let (sps_nal, pps_nal) = (self.sps.as_ref()?, self.pps.as_ref()?);
        if sps_nal.len() < 2 || pps_nal.len() < 2 {
            return None;
        }

        let parsed_sps = H265Sps::parse(sps_nal).ok()?;

        // Prefer VPS for the general profile/tier/level, fall back to SPS.
        let ptl = if let Some(vps_nal) = self.vps.as_ref() {
            H265Vps::parse(vps_nal).ok().map(|v| v.profile_tier_level)
        } else {
            None
        };
        let ptl = ptl.unwrap_or(parsed_sps.profile_tier_level);

        let vps_list = self
            .vps
            .as_ref()
            .map(|v| vec![v.to_vec()])
            .unwrap_or_default();
        let sps_list = vec![sps_nal.to_vec()];
        let pps_list = vec![pps_nal.to_vec()];

        let cfg = H265CodecConfig {
            configuration_version: 1,
            general_profile_space: ptl.general_profile_space,
            general_tier_flag: ptl.general_tier_flag,
            general_profile_idc: ptl.general_profile_idc,
            general_profile_compatibility_flags: ptl.general_profile_compatibility_flags,
            general_constraint_indicator_flags: ptl.general_constraint_indicator_flags,
            general_level_idc: ptl.general_level_idc,
            min_spatial_segmentation_idc: 0,
            parallelism_type: 0,
            chroma_format: parsed_sps.chroma_format_idc,
            bit_depth_luma_minus8: parsed_sps.bit_depth_luma_minus8,
            bit_depth_chroma_minus8: parsed_sps.bit_depth_chroma_minus8,
            avg_frame_rate: 0,
            constant_frame_rate: 0,
            num_temporal_layers: parsed_sps.max_sub_layers_minus1 + 1,
            temporal_id_nested: parsed_sps.temporal_id_nesting_flag,
            length_size_minus_one: 3,
            vps_list,
            sps_list,
            pps_list,
            codec_string: H265CodecConfig::build_codec_string(
                ptl.general_profile_space,
                ptl.general_tier_flag,
                ptl.general_profile_idc,
                ptl.general_profile_compatibility_flags,
                ptl.general_constraint_indicator_flags,
                ptl.general_level_idc,
            ),
        };

        Some(CodecConfig::HevcC(cfg.build().ok()?))
    }

    /// Build a `VideoFormat` from the cached SPS, if available and valid.
    pub fn video_format(&self) -> Option<VideoFormat> {
        let sps_nal = self.sps.as_ref()?;
        if sps_nal.len() < 2 {
            return None;
        }
        let parsed = H265Sps::parse(sps_nal).ok()?;
        let width = parsed.width();
        let height = parsed.height();
        Some(VideoFormat {
            pixel_format: Self::pixel_format(
                parsed.chroma_format_idc,
                parsed.separate_colour_plane_flag,
            ),
            coded_width: parsed.pic_width_in_luma_samples,
            coded_height: parsed.pic_height_in_luma_samples,
            visible_width: width,
            visible_height: height,
            stride: width,
            color_space: ColorSpace::Unspecified,
        })
    }

    /// Update a `TrackInfo` with the current HevcC config and video format.
    pub fn update_track(&self, track: &mut TrackInfo) {
        if let Some(config) = self.build_config() {
            track.set_codec_config(config);
        }
        if let Some(format) = self.video_format() {
            let _ = track.set_video_format(format);
        }
    }
}

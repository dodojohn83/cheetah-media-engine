//! H.264 parameter set cache and AvcC configuration generation.

use alloc::vec;
use alloc::vec::Vec;
use cheetah_media_bitstream::h264::{H264CodecConfig, Sps, unescape_rbsp};
use cheetah_media_types::{CodecConfig, ColorSpace, PixelFormat, TrackInfo, VideoFormat};

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

    /// Store a SPS NAL unit (header + RBSP, with emulation prevention bytes).
    pub fn set_sps(&mut self, nal: &[u8]) {
        self.sps = Some(nal.to_vec());
    }

    /// Store a PPS NAL unit (header + RBSP, with emulation prevention bytes).
    pub fn set_pps(&mut self, nal: &[u8]) {
        self.pps = Some(nal.to_vec());
    }

    /// Borrow the cached SPS, if any.
    pub fn sps(&self) -> Option<&[u8]> {
        self.sps.as_deref()
    }

    /// Borrow the cached PPS, if any.
    pub fn pps(&self) -> Option<&[u8]> {
        self.pps.as_deref()
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

    /// Build a `CodecConfig::AvcC` from the cached parameter sets, if complete.
    pub fn build_config(&self) -> Option<CodecConfig> {
        let (sps_nal, pps_nal) = (self.sps.as_ref()?, self.pps.as_ref()?);
        if sps_nal.is_empty() || pps_nal.is_empty() {
            return None;
        }
        let sps_rbsp = unescape_rbsp(&sps_nal[1..]);
        let parsed = Sps::parse(&sps_rbsp).ok()?;

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

        Some(CodecConfig::AvcC(cfg.build()))
    }

    /// Build a `VideoFormat` from the cached SPS, if available and valid.
    pub fn video_format(&self) -> Option<VideoFormat> {
        let sps_nal = self.sps.as_ref()?;
        if sps_nal.len() < 2 {
            return None;
        }
        let sps_rbsp = unescape_rbsp(&sps_nal[1..]);
        let parsed = Sps::parse(&sps_rbsp).ok()?;
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
            track.set_video_format(format).ok();
        }
    }
}

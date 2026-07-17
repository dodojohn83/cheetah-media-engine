//! Decoder capability description and selection constraints.

use alloc::vec::Vec;

use cheetah_media_types::CodecId;

/// Platform decoder API family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformApi {
    /// Windows Media Foundation.
    MediaFoundation,
    /// Apple VideoToolbox.
    VideoToolbox,
    /// Linux VA-API.
    VaApi,
    /// Vulkan Video decode queue.
    VulkanVideo,
    /// Pure software fallback.
    Software,
}

/// Whether a backend uses hardware, software, or a hybrid path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendKind {
    Hardware,
    Software,
    Hybrid,
}

/// Video decoder profile/level constraints for a single codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoProfileConstraint {
    pub profile: u32,
    pub level: u32,
    pub max_width: u32,
    pub max_height: u32,
    pub max_fps: u32,
    pub bit_depth: u8,
}

/// A video codec and the profiles it supports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoCodecSupport {
    pub codec: CodecId,
    pub profiles: Vec<VideoProfileConstraint>,
}

/// Capability reported by a single platform decoder probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderCapability {
    /// Which platform API provides this backend.
    pub api: PlatformApi,
    /// Hardware, software, or hybrid.
    pub kind: BackendKind,
    /// Supported video codecs and profile constraints.
    pub video_codecs: Vec<VideoCodecSupport>,
    /// Supported audio codecs.
    pub audio_codecs: Vec<CodecId>,
    /// Zero-copy surface formats supported, e.g. "nv12", "p010", "yuv420p".
    pub zero_copy_surfaces: Vec<&'static str>,
    /// Maximum concurrent decoder instances, if known.
    pub concurrent_instances: Option<u32>,
    /// Higher values are preferred when multiple backends support the codec.
    pub priority: i32,
}

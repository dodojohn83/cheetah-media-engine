//! Platform decoder capability probes.
//!
//! Each probe is intentionally conservative: it only reports capabilities that
//! can be verified at build or runtime. The platform-specific backends
//! (Media Foundation, VideoToolbox, VA-API, Vulkan Video) are currently stubs
//! that return no capabilities because the native SDKs are not linked in this
//! cross-platform CI build. Future PRs will implement real runtime probing.

use alloc::vec::Vec;

use cheetah_media_types::CodecId;

use crate::capability::{BackendKind, DecoderCapability, PlatformApi};

/// A decoder capability probe.
pub trait Probe {
    /// Human-readable probe name.
    fn name(&self) -> &'static str;
    /// Return all capabilities discovered by this probe.
    fn probe(&self) -> Vec<DecoderCapability>;
}

/// Software decoder probe.
///
/// Reports codecs for which a Rust software decoder is implemented in this
/// crate. It never claims support for paths that are not actually implemented.
pub struct SoftwareProbe;

impl Probe for SoftwareProbe {
    fn name(&self) -> &'static str {
        "software"
    }

    fn probe(&self) -> Vec<DecoderCapability> {
        vec![DecoderCapability {
            api: PlatformApi::Software,
            kind: BackendKind::Software,
            video_codecs: Vec::new(), // no software video decoder yet
            audio_codecs: vec![CodecId::G711A, CodecId::G711U],
            zero_copy_surfaces: Vec::new(),
            concurrent_instances: None,
            priority: 0,
        }]
    }
}

/// Windows Media Foundation probe (stub).
pub struct MediaFoundationProbe;

impl Probe for MediaFoundationProbe {
    fn name(&self) -> &'static str {
        "media-foundation"
    }

    fn probe(&self) -> Vec<DecoderCapability> {
        Vec::new()
    }
}

/// Apple VideoToolbox probe (stub).
pub struct VideoToolboxProbe;

impl Probe for VideoToolboxProbe {
    fn name(&self) -> &'static str {
        "videotoolbox"
    }

    fn probe(&self) -> Vec<DecoderCapability> {
        Vec::new()
    }
}

/// Linux VA-API probe (stub).
pub struct VaApiProbe;

impl Probe for VaApiProbe {
    fn name(&self) -> &'static str {
        "vaapi"
    }

    fn probe(&self) -> Vec<DecoderCapability> {
        Vec::new()
    }
}

/// Vulkan Video probe (stub).
pub struct VulkanVideoProbe;

impl Probe for VulkanVideoProbe {
    fn name(&self) -> &'static str {
        "vulkan-video"
    }

    fn probe(&self) -> Vec<DecoderCapability> {
        Vec::new()
    }
}

/// Convenience probe that aggregates the built-in platform probes.
pub struct DefaultProbe;

impl Probe for DefaultProbe {
    fn name(&self) -> &'static str {
        "default"
    }

    fn probe(&self) -> Vec<DecoderCapability> {
        let mut caps = Vec::new();
        caps.extend(MediaFoundationProbe.probe());
        caps.extend(VideoToolboxProbe.probe());
        caps.extend(VaApiProbe.probe());
        caps.extend(VulkanVideoProbe.probe());
        caps.extend(SoftwareProbe.probe());
        caps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_probe_reports_g711_only() {
        let cap = SoftwareProbe.probe().pop().unwrap();
        assert_eq!(cap.api, PlatformApi::Software);
        assert!(cap.audio_codecs.contains(&CodecId::G711A));
        assert!(cap.audio_codecs.contains(&CodecId::G711U));
        assert!(cap.video_codecs.is_empty());
    }

    #[test]
    fn default_probe_contains_software() {
        let caps = DefaultProbe.probe();
        assert!(caps.iter().any(|c| c.api == PlatformApi::Software));
    }
}

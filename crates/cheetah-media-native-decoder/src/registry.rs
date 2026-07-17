//! Capability registry and backend selection.

use alloc::vec::Vec;

use cheetah_media_types::CodecId;

use crate::capability::{DecoderCapability, PlatformApi};
use crate::probe::Probe;

/// Aggregated decoder capabilities from all platform probes.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CapabilityRegistry {
    entries: Vec<DecoderCapability>,
}

impl CapabilityRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add a manually-specified capability. Useful for tests and for
    /// overriding the built-in probes.
    pub fn add(&mut self, cap: DecoderCapability) {
        self.entries.push(cap);
    }

    /// Run a probe and register its capabilities.
    pub fn register<P: Probe>(&mut self, probe: P) {
        self.entries.extend(probe.probe());
    }

    /// Select the best backend for a video codec and resolution/fps constraint.
    /// Returns `None` if no registered backend reports support.
    pub fn select(&self, codec: CodecId, width: u32, height: u32, fps: u32) -> Option<PlatformApi> {
        let mut candidates: Vec<_> = self
            .entries
            .iter()
            .filter(|cap| supports_video(cap, codec, width, height, fps))
            .collect();
        candidates.sort_by_key(|cap| -cap.priority);
        candidates.first().map(|cap| cap.api)
    }

    /// Select the best backend for an audio codec.
    pub fn select_audio(&self, codec: CodecId) -> Option<PlatformApi> {
        let mut candidates: Vec<_> = self
            .entries
            .iter()
            .filter(|cap| cap.audio_codecs.contains(&codec))
            .collect();
        candidates.sort_by_key(|cap| -cap.priority);
        candidates.first().map(|cap| cap.api)
    }

    /// Return all registered capabilities.
    pub fn capabilities(&self) -> &[DecoderCapability] {
        &self.entries
    }
}

fn supports_video(
    cap: &DecoderCapability,
    codec: CodecId,
    width: u32,
    height: u32,
    fps: u32,
) -> bool {
    cap.video_codecs.iter().any(|vc| {
        if vc.codec != codec {
            return false;
        }
        // If no profiles are specified, reject the codec to avoid overclaiming.
        // Probes must provide explicit profile/level constraints.
        if vc.profiles.is_empty() {
            return false;
        }
        vc.profiles
            .iter()
            .any(|p| width <= p.max_width && height <= p.max_height && fps <= p.max_fps)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{BackendKind, VideoCodecSupport, VideoProfileConstraint};

    fn make_video_cap(api: PlatformApi, priority: i32, max_width: u32) -> DecoderCapability {
        DecoderCapability {
            api,
            kind: BackendKind::Hardware,
            video_codecs: vec![VideoCodecSupport {
                codec: CodecId::H264,
                profiles: vec![VideoProfileConstraint {
                    profile: 100,
                    level: 41,
                    max_width,
                    max_height: max_width,
                    max_fps: 60,
                    bit_depth: 8,
                }],
            }],
            audio_codecs: Vec::new(),
            zero_copy_surfaces: Vec::new(),
            concurrent_instances: None,
            priority,
        }
    }

    #[test]
    fn select_picks_highest_priority_backend() {
        let mut reg = CapabilityRegistry::new();
        reg.add(make_video_cap(PlatformApi::VaApi, 10, 1920));
        reg.add(make_video_cap(PlatformApi::VulkanVideo, 20, 1920));
        assert_eq!(
            reg.select(CodecId::H264, 1920, 1080, 30),
            Some(PlatformApi::VulkanVideo)
        );
    }

    #[test]
    fn select_respects_resolution_constraint() {
        let mut reg = CapabilityRegistry::new();
        reg.add(make_video_cap(PlatformApi::VaApi, 10, 1280));
        reg.add(make_video_cap(PlatformApi::VulkanVideo, 20, 1920));
        assert_eq!(
            reg.select(CodecId::H264, 1920, 1080, 30),
            Some(PlatformApi::VulkanVideo)
        );
    }

    #[test]
    fn select_returns_none_when_constraints_exceeded() {
        let mut reg = CapabilityRegistry::new();
        reg.add(make_video_cap(PlatformApi::VaApi, 10, 1280));
        assert_eq!(reg.select(CodecId::H264, 1920, 1080, 30), None);
    }

    #[test]
    fn select_audio_prefers_higher_priority() {
        let mut reg = CapabilityRegistry::new();
        reg.add(DecoderCapability {
            api: PlatformApi::Software,
            kind: BackendKind::Software,
            video_codecs: Vec::new(),
            audio_codecs: vec![CodecId::G711A],
            zero_copy_surfaces: Vec::new(),
            concurrent_instances: None,
            priority: 5,
        });
        assert_eq!(
            reg.select_audio(CodecId::G711A),
            Some(PlatformApi::Software)
        );
    }
}

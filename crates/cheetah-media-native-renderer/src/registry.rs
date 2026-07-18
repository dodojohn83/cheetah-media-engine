//! Renderer capability registry and backend selection.

use alloc::vec::Vec;

use crate::capability::{PixelFormat, PlatformRenderer, RendererCapability};
use crate::probe::RendererProbe;

/// Aggregated renderer capabilities from all platform probes.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RendererRegistry {
    entries: Vec<RendererCapability>,
}

impl RendererRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Create a registry pre-populated from a single probe.
    pub fn with_probe<P: RendererProbe>(probe: P) -> Self {
        let mut reg = Self::new();
        reg.register(probe);
        reg
    }

    /// Add a manually-specified capability.
    pub fn add(&mut self, cap: RendererCapability) {
        self.entries.push(cap);
    }

    /// Run a probe and register its capabilities.
    pub fn register<P: RendererProbe>(&mut self, probe: P) {
        self.entries.extend(probe.probe());
    }

    /// Select the best renderer API for a given format and resolution.
    /// Returns `None` if no registered renderer reports support.
    pub fn select(&self, format: PixelFormat, width: u32, height: u32) -> Option<PlatformRenderer> {
        let mut candidates: Vec<_> = self
            .entries
            .iter()
            .filter(|cap| supports_format(cap, format, width, height))
            .collect();
        candidates.sort_by(|a, b| b.priority.cmp(&a.priority));
        candidates.first().map(|cap| cap.api)
    }

    /// Return all registered capabilities.
    pub fn capabilities(&self) -> &[RendererCapability] {
        &self.entries
    }
}

fn supports_format(cap: &RendererCapability, format: PixelFormat, width: u32, height: u32) -> bool {
    cap.formats
        .iter()
        .any(|f| f.format == format && width <= f.max_width && height <= f.max_height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::RendererFormatSupport;

    #[test]
    fn select_prefers_higher_priority() {
        let mut reg = RendererRegistry::new();
        reg.add(RendererCapability {
            api: PlatformRenderer::Software,
            formats: vec![RendererFormatSupport {
                format: PixelFormat::Rgba32,
                max_width: 1920,
                max_height: 1080,
                zero_copy: false,
            }],
            priority: 0,
        });
        reg.add(RendererCapability {
            api: PlatformRenderer::OpenGl,
            formats: vec![RendererFormatSupport {
                format: PixelFormat::Rgba32,
                max_width: 1920,
                max_height: 1080,
                zero_copy: true,
            }],
            priority: 10,
        });
        assert_eq!(
            reg.select(PixelFormat::Rgba32, 1280, 720),
            Some(PlatformRenderer::OpenGl)
        );
    }

    #[test]
    fn select_respects_resolution_limit() {
        let mut reg = RendererRegistry::new();
        reg.add(RendererCapability {
            api: PlatformRenderer::Software,
            formats: vec![RendererFormatSupport {
                format: PixelFormat::Rgba32,
                max_width: 1280,
                max_height: 720,
                zero_copy: false,
            }],
            priority: 0,
        });
        assert_eq!(reg.select(PixelFormat::Rgba32, 1920, 1080), None);
    }
}

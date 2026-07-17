//! Platform renderer capability probes.
//!
//! GPU renderer probes are currently stubs returning no capabilities until the
//! target platform graphics SDKs are linked. The Software probe reports a
//! conservative CPU fallback so the `NativeRenderer` can always render to an
//! in-memory surface.

use alloc::vec::Vec;

use crate::capability::{PixelFormat, PlatformRenderer, RendererCapability, RendererFormatSupport};

/// A renderer capability probe.
pub trait RendererProbe {
    /// Human-readable probe name.
    fn name(&self) -> &'static str;
    /// Return all capabilities discovered by this probe.
    fn probe(&self) -> Vec<RendererCapability>;
}

/// CPU software renderer probe.
pub struct SoftwareRendererProbe;

impl RendererProbe for SoftwareRendererProbe {
    fn name(&self) -> &'static str {
        "software-renderer"
    }

    fn probe(&self) -> Vec<RendererCapability> {
        vec![RendererCapability {
            api: PlatformRenderer::Software,
            formats: vec![
                RendererFormatSupport {
                    format: PixelFormat::Rgba32,
                    max_width: 3840,
                    max_height: 2160,
                    zero_copy: false,
                },
                RendererFormatSupport {
                    format: PixelFormat::Rgb24,
                    max_width: 3840,
                    max_height: 2160,
                    zero_copy: false,
                },
                RendererFormatSupport {
                    format: PixelFormat::I420,
                    max_width: 3840,
                    max_height: 2160,
                    zero_copy: false,
                },
                RendererFormatSupport {
                    format: PixelFormat::Nv12,
                    max_width: 3840,
                    max_height: 2160,
                    zero_copy: false,
                },
            ],
            priority: 0,
        }]
    }
}

/// OpenGL / OpenGL ES probe (stub).
pub struct OpenGlRendererProbe;

impl RendererProbe for OpenGlRendererProbe {
    fn name(&self) -> &'static str {
        "opengl"
    }

    fn probe(&self) -> Vec<RendererCapability> {
        Vec::new()
    }
}

/// Vulkan renderer probe (stub).
pub struct VulkanRendererProbe;

impl RendererProbe for VulkanRendererProbe {
    fn name(&self) -> &'static str {
        "vulkan"
    }

    fn probe(&self) -> Vec<RendererCapability> {
        Vec::new()
    }
}

/// Metal renderer probe (stub).
pub struct MetalRendererProbe;

impl RendererProbe for MetalRendererProbe {
    fn name(&self) -> &'static str {
        "metal"
    }

    fn probe(&self) -> Vec<RendererCapability> {
        Vec::new()
    }
}

/// D3D11 renderer probe (stub).
pub struct D3D11RendererProbe;

impl RendererProbe for D3D11RendererProbe {
    fn name(&self) -> &'static str {
        "d3d11"
    }

    fn probe(&self) -> Vec<RendererCapability> {
        Vec::new()
    }
}

/// Convenience probe that aggregates the built-in renderer probes.
pub struct DefaultRendererProbe;

impl RendererProbe for DefaultRendererProbe {
    fn name(&self) -> &'static str {
        "default"
    }

    fn probe(&self) -> Vec<RendererCapability> {
        let mut caps = Vec::new();
        caps.extend(OpenGlRendererProbe.probe());
        caps.extend(VulkanRendererProbe.probe());
        caps.extend(MetalRendererProbe.probe());
        caps.extend(D3D11RendererProbe.probe());
        caps.extend(SoftwareRendererProbe.probe());
        caps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_probe_reports_rgba_and_rgb() {
        let cap = SoftwareRendererProbe.probe().pop().unwrap();
        assert_eq!(cap.api, PlatformRenderer::Software);
        assert!(cap.formats.iter().any(|f| f.format == PixelFormat::Rgba32));
        assert!(cap.formats.iter().any(|f| f.format == PixelFormat::Rgb24));
    }

    #[test]
    fn default_probe_contains_software() {
        let caps = DefaultRendererProbe.probe();
        assert!(caps.iter().any(|c| c.api == PlatformRenderer::Software));
    }
}

//! Renderer capability description and selection constraints.

use alloc::vec::Vec;

/// Native rendering API family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformRenderer {
    /// OpenGL / OpenGL ES.
    OpenGl,
    /// Vulkan.
    Vulkan,
    /// Apple Metal.
    Metal,
    /// Direct3D 11.
    D3D11,
    /// CPU memory fallback.
    Software,
}

/// Pixel format supported by a renderer surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// 32-bit RGBA, 8 bits per channel.
    Rgba32,
    /// 24-bit RGB, 8 bits per channel.
    Rgb24,
    /// 4:2:0 planar YUV (Y, then U, then V).
    I420,
    /// 4:2:0 semi-planar YUV (Y, then interleaved UV).
    Nv12,
}

/// Resolution and format support for a single renderer capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererFormatSupport {
    pub format: PixelFormat,
    pub max_width: u32,
    pub max_height: u32,
    pub zero_copy: bool,
}

/// Capability reported by a single platform renderer probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererCapability {
    /// Which platform API provides this renderer.
    pub api: PlatformRenderer,
    /// Supported pixel formats and resolution limits.
    pub formats: Vec<RendererFormatSupport>,
    /// Higher values are preferred when multiple renderers support a format.
    pub priority: i32,
}

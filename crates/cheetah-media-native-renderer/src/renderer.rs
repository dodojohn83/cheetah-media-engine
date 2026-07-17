//! `NativeRenderer` with CPU fallback and zero-copy surface access.

use alloc::boxed::Box;

use cheetah_media_abi::{AbiError, Output, Renderer};

use crate::capability::{PixelFormat, PlatformRenderer};
use crate::registry::RendererRegistry;
use crate::surface::Surface;

/// Access to the current rendered surface.
pub trait SurfaceAccess {
    /// Return the most recently rendered surface, if any.
    fn surface(&self) -> Option<&Surface>;
}

/// A renderer backend selected from the capability registry.
pub struct NativeRenderer {
    backend: Box<dyn RendererBackend + Send>,
}

impl NativeRenderer {
    /// Create a renderer from a registry and requested format.
    ///
    /// Falls back to the software CPU renderer if no GPU backend reports
    /// support for `format` and `width`/`height`.
    pub fn from_registry(
        registry: &RendererRegistry,
        format: PixelFormat,
        width: u32,
        height: u32,
    ) -> Result<Self, AbiError> {
        let selected = registry
            .select(format, width, height)
            .unwrap_or(PlatformRenderer::Software);

        let backend: Box<dyn RendererBackend + Send> = match selected {
            PlatformRenderer::Software => Box::new(CpuRenderer::new(width, height, format)),
            _ => Box::new(UnsupportedRenderer::new(selected)),
        };

        Ok(Self { backend })
    }

    /// Create a renderer with an explicit backend. Useful for tests.
    pub fn with_backend(backend: Box<dyn RendererBackend + Send>) -> Self {
        Self { backend }
    }
}

impl Renderer for NativeRenderer {
    fn render(&mut self, output: &Output) -> Result<(), AbiError> {
        self.backend.render(output)
    }

    fn set_viewport(&mut self, width: u32, height: u32) -> Result<(), AbiError> {
        self.backend.set_viewport(width, height)
    }
}

impl SurfaceAccess for NativeRenderer {
    fn surface(&self) -> Option<&Surface> {
        self.backend.surface()
    }
}

/// Internal renderer backend trait.
pub trait RendererBackend: Renderer + SurfaceAccess {}

/// CPU renderer that stores the latest frame in a `Surface`.
pub struct CpuRenderer {
    surface: Surface,
}

impl CpuRenderer {
    /// Create a CPU renderer with the given initial dimensions and format.
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        Self {
            surface: Surface::new(width, height, format),
        }
    }
}

impl Renderer for CpuRenderer {
    fn render(&mut self, output: &Output) -> Result<(), AbiError> {
        let expected = self.surface.expected_size();
        if output.data.len() != expected {
            return Err(AbiError::InvalidData);
        }
        self.surface
            .upload(&output.data)
            .map_err(|_| AbiError::InvalidData)
    }

    fn set_viewport(&mut self, width: u32, height: u32) -> Result<(), AbiError> {
        if width == 0 || height == 0 {
            return Err(AbiError::InvalidData);
        }
        self.surface = Surface::new(width, height, self.surface.format);
        Ok(())
    }
}

impl SurfaceAccess for CpuRenderer {
    fn surface(&self) -> Option<&Surface> {
        if self.surface.data.is_empty() {
            return None;
        }
        Some(&self.surface)
    }
}

impl RendererBackend for CpuRenderer {}

/// Stub renderer for a GPU backend that has not been linked yet.
pub struct UnsupportedRenderer {
    _api: PlatformRenderer,
}

impl UnsupportedRenderer {
    /// Create a stub for the given platform API.
    pub fn new(api: PlatformRenderer) -> Self {
        Self { _api: api }
    }
}

impl Renderer for UnsupportedRenderer {
    fn render(&mut self, _output: &Output) -> Result<(), AbiError> {
        Err(AbiError::NotSupported)
    }

    fn set_viewport(&mut self, _width: u32, _height: u32) -> Result<(), AbiError> {
        Err(AbiError::NotSupported)
    }
}

impl SurfaceAccess for UnsupportedRenderer {
    fn surface(&self) -> Option<&Surface> {
        None
    }
}

impl RendererBackend for UnsupportedRenderer {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::SoftwareRendererProbe;
    use cheetah_media_types::{MediaTime, TimeBase, Timestamp, TrackId};

    use alloc::vec::Vec;

    fn output(data: Vec<u8>) -> Output {
        Output {
            data,
            time: MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT),
            duration_ms: 0,
            track_id: TrackId::new(1).unwrap(),
        }
    }

    #[test]
    fn cpu_renderer_uploads_rgba_frame() {
        let mut renderer = CpuRenderer::new(2, 2, PixelFormat::Rgba32);
        let data = vec![0u8; 16];
        assert!(renderer.render(&output(data)).is_ok());
        assert!(renderer.surface().is_some());
        assert_eq!(renderer.surface().unwrap().data.len(), 16);
    }

    #[test]
    fn cpu_renderer_rejects_wrong_size() {
        let mut renderer = CpuRenderer::new(2, 2, PixelFormat::Rgba32);
        let data = vec![0u8; 15];
        assert_eq!(
            renderer.render(&output(data)).unwrap_err(),
            AbiError::InvalidData
        );
    }

    #[test]
    fn cpu_renderer_set_viewport_resizes_surface() {
        let mut renderer = CpuRenderer::new(2, 2, PixelFormat::Rgba32);
        renderer.set_viewport(4, 3).unwrap();
        let data = vec![0u8; 48]; // 4 * 3 * 4
        assert!(renderer.render(&output(data)).is_ok());
    }

    #[test]
    fn native_renderer_from_registry_uses_software_fallback() {
        let reg = RendererRegistry::with_probe(SoftwareRendererProbe);
        let mut renderer =
            NativeRenderer::from_registry(&reg, PixelFormat::Rgba32, 1920, 1080).unwrap();
        let data = vec![0u8; 1920 * 1080 * 4];
        assert!(renderer.render(&output(data)).is_ok());
        assert!(renderer.surface().is_some());
    }

    #[test]
    fn unsupported_renderer_returns_not_supported() {
        let mut renderer = UnsupportedRenderer::new(PlatformRenderer::Vulkan);
        assert_eq!(
            renderer.render(&output(vec![])).unwrap_err(),
            AbiError::NotSupported
        );
    }
}

//! Android `Surface`/`SurfaceView` renderer implementation.
//!
//! Host-side stub: rendering requires an Android surface and will be wired in
//! WP-64. It reports `AbiError::NotSupported` so the capability registry does
//! not accidentally select it on non-Android targets.

use cheetah_media_abi::{AbiError, Output, Renderer};

/// Renderer that targets an Android `Surface`.
pub struct AndroidRenderer;

impl AndroidRenderer {
    /// Create a new Android renderer.
    pub const fn new() -> Self {
        Self
    }
}

impl Default for AndroidRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for AndroidRenderer {
    fn render(&mut self, _output: &Output) -> Result<(), AbiError> {
        Err(AbiError::NotSupported)
    }

    fn set_viewport(&mut self, _width: u32, _height: u32) -> Result<(), AbiError> {
        Err(AbiError::NotSupported)
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;
    use cheetah_media_abi::Output;
    use cheetah_media_types::{MediaTime, TimeBase, Timestamp, TrackId};

    #[test]
    fn host_stub_rejects_render_and_viewport() {
        let mut renderer = AndroidRenderer::new();
        let output = Output {
            data: Vec::new(),
            time: MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT),
            duration_ms: 0,
            track_id: TrackId::new(1).unwrap(),
        };
        assert_eq!(
            renderer.render(&output).unwrap_err(),
            AbiError::NotSupported
        );
        assert_eq!(
            renderer.set_viewport(1920, 1080).unwrap_err(),
            AbiError::NotSupported
        );
    }
}

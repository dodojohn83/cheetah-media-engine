//! Native decoded frame surface.

use alloc::vec::Vec;

use crate::capability::PixelFormat;

/// A decoded frame in native memory.
///
/// `data` is owned for CPU renderers. GPU renderers would instead store an
/// opaque handle (e.g. texture name, `CVPixelBuffer` reference) and leave
/// `data` empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surface {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: PixelFormat,
    pub data: Vec<u8>,
}

impl Surface {
    /// Create an empty surface with the given dimensions and format.
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        let stride = width * bytes_per_pixel(format);
        Self {
            width,
            height,
            stride,
            format,
            data: Vec::new(),
        }
    }

    /// Expected byte size for the current dimensions and format.
    ///
    /// For packed RGB/RGBA this is `height * stride`. For planar YUV formats
    /// it includes the luma plane plus the chroma planes.
    pub fn expected_size(&self) -> usize {
        match self.format {
            PixelFormat::Rgba32 | PixelFormat::Rgb24 => self.height as usize * self.stride as usize,
            PixelFormat::I420 => {
                let y = self.width as usize * self.height as usize;
                let c = self.width.div_ceil(2) as usize * self.height.div_ceil(2) as usize * 2;
                y + c
            }
            PixelFormat::Nv12 => {
                let y = self.width as usize * self.height as usize;
                let uv = self.width as usize * self.height.div_ceil(2) as usize;
                y + uv
            }
        }
    }

    /// Copy `data` into the surface, validating size.
    pub fn upload(&mut self, data: &[u8]) -> Result<(), UploadError> {
        let expected = self.expected_size();
        if data.len() != expected {
            return Err(UploadError {
                expected,
                actual: data.len(),
            });
        }
        self.data.clear();
        self.data.extend_from_slice(data);
        Ok(())
    }
}

/// Error returned when the uploaded data does not match the surface format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadError {
    pub expected: usize,
    pub actual: usize,
}

impl core::fmt::Display for UploadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "surface upload size mismatch: expected {} bytes, got {}",
            self.expected, self.actual
        )
    }
}

const fn bytes_per_pixel(format: PixelFormat) -> u32 {
    match format {
        PixelFormat::Rgba32 => 4,
        PixelFormat::Rgb24 => 3,
        // I420/Nv12 use 1.5 bytes per pixel on average; we use the luma stride
        // (1 byte per luma sample) as the surface row stride.
        PixelFormat::I420 | PixelFormat::Nv12 => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_surface_expected_size() {
        let s = Surface::new(2, 3, PixelFormat::Rgba32);
        assert_eq!(s.stride, 8);
        assert_eq!(s.expected_size(), 24);
    }

    #[test]
    fn upload_wrong_size_fails() {
        let mut s = Surface::new(2, 2, PixelFormat::Rgba32);
        assert!(s.upload(&[0u8; 15]).is_err());
    }

    #[test]
    fn upload_correct_size_succeeds() {
        let mut s = Surface::new(2, 2, PixelFormat::Rgba32);
        assert!(s.upload(&[0u8; 16]).is_ok());
        assert_eq!(s.data.len(), 16);
    }

    #[test]
    fn i420_expected_size_includes_chroma() {
        // 4x4 I420: 16 Y + 4 U + 4 V = 24
        let s = Surface::new(4, 4, PixelFormat::I420);
        assert_eq!(s.expected_size(), 24);
    }

    #[test]
    fn nv12_expected_size_includes_chroma() {
        // 4x4 NV12: 16 Y + 8 UV = 24
        let s = Surface::new(4, 4, PixelFormat::Nv12);
        assert_eq!(s.expected_size(), 24);
    }
}

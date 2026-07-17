//! Decoded video and audio frames.

use alloc::vec::Vec;

use crate::{
    AudioFormat, BufferLifecycle, BufferRef, MediaError, MediaLimits, MediaTime, VideoFormat,
};

fn total_payload_len(payload: &BufferRef<'_>, planes: &[BufferRef<'_>]) -> u64 {
    let mut total = payload.len();
    for p in planes {
        total = total.saturating_add(p.len());
    }
    total as u64
}

/// Opaque external frame resource handle.
///
/// A handle value of `0` means no external resource. This keeps platform pointers
/// out of the type system and lets renderers map handles to textures/surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ExternalFrameHandle(u64);

impl ExternalFrameHandle {
    pub const fn new(handle: u64) -> Self {
        Self(handle)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn is_none(self) -> bool {
        self.0 == 0
    }
}

/// A decoded video frame.
///
/// Planar frames store each plane as a `BufferRef`; interleaved frames keep all
/// data in `payload` with `planes` empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame<'a> {
    pub payload: BufferRef<'a>,
    pub planes: Vec<BufferRef<'a>>,
    pub format: VideoFormat,
    pub timestamp: MediaTime,
    pub handle: Option<ExternalFrameHandle>,
}

impl<'a> VideoFrame<'a> {
    /// Create an owned video frame from a single contiguous buffer.
    pub fn new(
        payload: impl Into<BufferRef<'a>>,
        format: VideoFormat,
        timestamp: MediaTime,
    ) -> Self {
        Self {
            payload: payload.into(),
            planes: Vec::new(),
            format,
            timestamp,
            handle: None,
        }
    }

    /// Add a separate plane slice for planar formats.
    pub fn with_plane(mut self, plane: impl Into<BufferRef<'a>>) -> Self {
        self.planes.push(plane.into());
        self
    }

    /// Compute the minimum required buffer size for the coded dimensions and
    /// stride. This is a conservative estimate for common 8-bit formats.
    pub fn min_required_size(&self) -> u64 {
        let planes = if self.planes.is_empty() {
            1u64
        } else {
            self.planes.len() as u64
        };
        let height = u64::from(self.format.coded_height);
        let stride = u64::from(self.format.stride);
        planes
            .checked_mul(height)
            .and_then(|v| v.checked_mul(stride))
            .unwrap_or(u64::MAX)
    }

    /// Validate the frame against resource limits.
    pub fn check_limits(&self, limits: &MediaLimits) -> Result<(), MediaError> {
        limits.check_resolution(self.format.visible_width, self.format.visible_height)?;
        let total = total_payload_len(&self.payload, &self.planes);
        if total > limits.max_frame_size_bytes {
            return Err(MediaError::ResourceLimit {
                name: "frame_size_bytes",
                current: total,
                limit: limits.max_frame_size_bytes,
            });
        }
        Ok(())
    }

    /// Lifetime classification of the frame buffer.
    pub fn lifecycle(&self) -> BufferLifecycle {
        if self.handle.is_some() {
            BufferLifecycle::External
        } else {
            self.payload.lifecycle()
        }
    }
}

/// A decoded audio frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFrame<'a> {
    pub payload: BufferRef<'a>,
    pub planes: Vec<BufferRef<'a>>,
    pub format: AudioFormat,
    pub timestamp: MediaTime,
    pub handle: Option<ExternalFrameHandle>,
}

impl<'a> AudioFrame<'a> {
    pub fn new(
        payload: impl Into<BufferRef<'a>>,
        format: AudioFormat,
        timestamp: MediaTime,
    ) -> Self {
        Self {
            payload: payload.into(),
            planes: Vec::new(),
            format,
            timestamp,
            handle: None,
        }
    }

    pub fn with_plane(mut self, plane: impl Into<BufferRef<'a>>) -> Self {
        self.planes.push(plane.into());
        self
    }

    /// Expected byte size for the configured sample count and layout.
    pub fn expected_size(&self) -> u64 {
        self.format
            .total_samples()
            .saturating_mul(u64::from(self.format.bytes_per_sample()))
    }

    pub fn check_limits(&self, limits: &MediaLimits) -> Result<(), MediaError> {
        let total = total_payload_len(&self.payload, &self.planes);
        if total > limits.max_frame_size_bytes {
            return Err(MediaError::ResourceLimit {
                name: "frame_size_bytes",
                current: total,
                limit: limits.max_frame_size_bytes,
            });
        }
        Ok(())
    }

    /// Lifetime classification of the frame buffer.
    pub fn lifecycle(&self) -> BufferLifecycle {
        if self.handle.is_some() {
            BufferLifecycle::External
        } else {
            self.payload.lifecycle()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChannelLayout, TimeBase, Timestamp};
    use crate::{ColorSpace, PixelFormat, SampleFormat};

    #[test]
    fn video_frame_min_size() {
        let fmt = VideoFormat {
            pixel_format: PixelFormat::Yuv420P,
            coded_width: 1280,
            coded_height: 720,
            visible_width: 1280,
            visible_height: 720,
            stride: 1280,
            color_space: ColorSpace::Bt709,
        };
        let ts = MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
        let frame = VideoFrame::new(vec![0u8; 1280 * 720], fmt, ts);
        assert_eq!(frame.min_required_size(), 921_600);
    }

    #[test]
    fn video_frame_with_planes() {
        let fmt = VideoFormat {
            pixel_format: PixelFormat::Yuv420P,
            coded_width: 1280,
            coded_height: 720,
            visible_width: 1280,
            visible_height: 720,
            stride: 1280,
            color_space: ColorSpace::Bt709,
        };
        let ts = MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
        let frame = VideoFrame::new(vec![0u8; 0], fmt, ts)
            .with_plane(vec![0u8; 100])
            .with_plane(vec![0u8; 50]);
        assert_eq!(frame.planes.len(), 2);
        assert_eq!(frame.min_required_size(), 2 * 720 * 1280);
    }

    #[test]
    fn video_frame_limit_rejects_oversized() {
        let fmt = VideoFormat {
            pixel_format: PixelFormat::Yuv420P,
            coded_width: 8192,
            coded_height: 4320,
            visible_width: 8192,
            visible_height: 4320,
            stride: 8192,
            color_space: ColorSpace::Bt709,
        };
        let ts = MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
        let frame = VideoFrame::new(vec![0u8; 1], fmt, ts);
        let limits = MediaLimits::default();
        let err = frame.check_limits(&limits).unwrap_err();
        assert_eq!(err.code(), 5001);
    }

    #[test]
    fn video_frame_planes_count_toward_limit() {
        let fmt = VideoFormat {
            pixel_format: PixelFormat::Yuv420P,
            coded_width: 64,
            coded_height: 64,
            visible_width: 64,
            visible_height: 64,
            stride: 64,
            color_space: ColorSpace::Bt709,
        };
        let ts = MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
        // Empty payload but planes exceed the configured max_frame_size_bytes.
        let frame = VideoFrame::new(vec![0u8; 0], fmt, ts)
            .with_plane(vec![0u8; 6])
            .with_plane(vec![0u8; 6]);
        let limits = MediaLimits {
            max_frame_size_bytes: 10,
            ..Default::default()
        };
        assert!(frame.check_limits(&limits).is_err());
    }

    #[test]
    fn audio_frame_planes_count_toward_limit() {
        let fmt = AudioFormat {
            sample_format: SampleFormat::S16,
            sample_rate: 44100,
            channel_layout: ChannelLayout::Mono,
            sample_count: 1,
        };
        let ts = MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
        let frame = AudioFrame::new(vec![0u8; 0], fmt, ts)
            .with_plane(vec![0u8; 6])
            .with_plane(vec![0u8; 6]);
        let limits = MediaLimits {
            max_frame_size_bytes: 10,
            ..Default::default()
        };
        assert!(frame.check_limits(&limits).is_err());
    }

    #[test]
    fn audio_expected_size_with_max_channels_does_not_overflow() {
        // Unknown(u64::MAX) reports 64 channels via count_ones. With the
        // maximum sample_count and F64 (8 bytes) the product is large but
        // still well within u64; the important check is that no panic occurs.
        let fmt = AudioFormat {
            sample_format: SampleFormat::F64,
            sample_rate: 48000,
            channel_layout: ChannelLayout::Unknown(u64::MAX),
            sample_count: u32::MAX,
        };
        let ts = MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
        let frame = AudioFrame::new(vec![0u8; 0], fmt, ts);
        let expected = (u32::MAX as u64) * 64 * 8;
        assert_eq!(frame.expected_size(), expected);
    }

    #[test]
    fn video_frame_external_lifecycle() {
        let fmt = VideoFormat {
            pixel_format: PixelFormat::Rgba,
            coded_width: 1,
            coded_height: 1,
            visible_width: 1,
            visible_height: 1,
            stride: 4,
            color_space: ColorSpace::Bt709,
        };
        let ts = MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
        let mut frame = VideoFrame::new(vec![0u8; 4], fmt, ts);
        assert_eq!(frame.lifecycle(), BufferLifecycle::Shared);
        frame.handle = Some(ExternalFrameHandle::new(1));
        assert_eq!(frame.lifecycle(), BufferLifecycle::External);
    }
}

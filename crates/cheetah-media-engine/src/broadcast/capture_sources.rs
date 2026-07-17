//! Concrete capture source implementations for the broadcast pipeline.
//!
//! WP-71 adds typed placeholder sources for camera, microphone and screen. Real
//! platform backends will replace these host stubs in later work packages.

use alloc::collections::VecDeque;

use cheetah_media_types::{MediaError, MediaTime, Timestamp};

use crate::broadcast::frame::MediaFrame;
use crate::broadcast::permission::CaptureSourceKind;
use crate::broadcast::source::CaptureSource;

/// Frame generation helper used by `MockCaptureSource`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoFrameInfo {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixel_format: cheetah_media_types::PixelFormat,
    pub color_space: cheetah_media_types::ColorSpace,
}

/// Placeholder camera capture source.
pub struct CameraCaptureSource {
    /// Requested capture resolution.
    pub width: u32,
    pub height: u32,
}

impl CaptureSource for CameraCaptureSource {
    fn start(&mut self) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7101,
            context: Some("camera capture source not linked"),
        })
    }

    fn stop(&mut self) -> Result<(), MediaError> {
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<MediaFrame<'static>>, MediaError> {
        Err(MediaError::Unsupported {
            code: 7101,
            context: Some("camera capture source not linked"),
        })
    }

    fn kind(&self) -> &'static str {
        "camera"
    }

    fn required_permission(&self) -> Option<CaptureSourceKind> {
        Some(CaptureSourceKind::Camera)
    }
}

/// Placeholder microphone/audio capture source.
pub struct MicrophoneCaptureSource;

impl CaptureSource for MicrophoneCaptureSource {
    fn start(&mut self) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7102,
            context: Some("microphone capture source not linked"),
        })
    }

    fn stop(&mut self) -> Result<(), MediaError> {
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<MediaFrame<'static>>, MediaError> {
        Err(MediaError::Unsupported {
            code: 7102,
            context: Some("microphone capture source not linked"),
        })
    }

    fn kind(&self) -> &'static str {
        "microphone"
    }

    fn required_permission(&self) -> Option<CaptureSourceKind> {
        Some(CaptureSourceKind::Microphone)
    }
}

/// Placeholder screen capture source.
pub struct ScreenCaptureSource;

impl CaptureSource for ScreenCaptureSource {
    fn start(&mut self) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7103,
            context: Some("screen capture source not linked"),
        })
    }

    fn stop(&mut self) -> Result<(), MediaError> {
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<MediaFrame<'static>>, MediaError> {
        Err(MediaError::Unsupported {
            code: 7103,
            context: Some("screen capture source not linked"),
        })
    }

    fn kind(&self) -> &'static str {
        "screen"
    }

    fn required_permission(&self) -> Option<CaptureSourceKind> {
        Some(CaptureSourceKind::Screen)
    }
}

/// Mock capture source for headless tests.
pub struct MockCaptureSource {
    frames: VecDeque<MediaFrame<'static>>,
    started: bool,
    video_info: VideoFrameInfo,
}

impl MockCaptureSource {
    /// Create a mock source that yields the given `frames` in insertion order.
    pub fn new(frames: alloc::vec::Vec<MediaFrame<'static>>, video_info: VideoFrameInfo) -> Self {
        Self {
            frames: VecDeque::from(frames),
            started: false,
            video_info,
        }
    }

    /// Frame format used to generate frames.
    pub fn info(&self) -> VideoFrameInfo {
        self.video_info
    }

    /// Create a mock source that yields `count` identical RGBA frames.
    pub fn with_count(count: usize, video_info: VideoFrameInfo) -> Self {
        use alloc::vec;
        // For identical frames order does not matter; for distinct frames `new`
        // preserves insertion order by consuming from the front of a VecDeque.
        let format = cheetah_media_types::VideoFormat {
            pixel_format: video_info.pixel_format,
            coded_width: video_info.width,
            coded_height: video_info.height,
            visible_width: video_info.width,
            visible_height: video_info.height,
            stride: video_info.stride,
            color_space: video_info.color_space,
        };
        let payload = vec![0u8; (video_info.stride * video_info.height) as usize];
        let ts = Timestamp::new(0);
        let frame = MediaFrame::Video(cheetah_media_types::VideoFrame::new(
            payload,
            format,
            MediaTime::from_pts_dts(ts, ts, cheetah_media_types::TimeBase::DEFAULT),
        ));
        Self::new(vec![frame; count], video_info)
    }
}

impl CaptureSource for MockCaptureSource {
    fn start(&mut self) -> Result<(), MediaError> {
        self.started = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), MediaError> {
        self.started = false;
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<MediaFrame<'static>>, MediaError> {
        if !self.started {
            return Ok(None);
        }
        Ok(self.frames.pop_front())
    }

    fn kind(&self) -> &'static str {
        "mock"
    }

    fn required_permission(&self) -> Option<CaptureSourceKind> {
        Some(CaptureSourceKind::Camera)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_types::{ColorSpace, PixelFormat};

    fn info() -> VideoFrameInfo {
        VideoFrameInfo {
            width: 2,
            height: 2,
            stride: 8,
            pixel_format: PixelFormat::Rgba,
            color_space: ColorSpace::Bt709,
        }
    }

    #[test]
    fn camera_source_requires_camera_permission() {
        let mut source = CameraCaptureSource {
            width: 640,
            height: 480,
        };
        assert_eq!(
            source.required_permission(),
            Some(CaptureSourceKind::Camera)
        );
        assert!(source.start().is_err());
    }

    #[test]
    fn microphone_source_requires_microphone_permission() {
        let mut source = MicrophoneCaptureSource;
        assert_eq!(
            source.required_permission(),
            Some(CaptureSourceKind::Microphone)
        );
        assert!(source.start().is_err());
    }

    #[test]
    fn screen_source_requires_screen_permission() {
        let mut source = ScreenCaptureSource;
        assert_eq!(
            source.required_permission(),
            Some(CaptureSourceKind::Screen)
        );
        assert!(source.start().is_err());
    }

    #[test]
    fn mock_source_yields_frames_after_start() {
        let mut source = MockCaptureSource::with_count(3, info());
        assert!(source.poll().unwrap().is_none());
        source.start().unwrap();
        assert!(source.poll().unwrap().is_some());
        source.stop().unwrap();
        assert!(source.poll().unwrap().is_none());
    }

    #[test]
    fn mock_source_yields_frames_in_insertion_order() {
        use alloc::vec;
        let info = info();
        let format = cheetah_media_types::VideoFormat {
            pixel_format: info.pixel_format,
            coded_width: info.width,
            coded_height: info.height,
            visible_width: info.width,
            visible_height: info.height,
            stride: info.stride,
            color_space: info.color_space,
        };
        let mut frames = vec![];
        for i in 0..3 {
            let payload = vec![i as u8; (info.stride * info.height) as usize];
            let ts = Timestamp::new(i);
            frames.push(MediaFrame::Video(cheetah_media_types::VideoFrame::new(
                payload,
                format,
                MediaTime::from_pts_dts(ts, ts, cheetah_media_types::TimeBase::DEFAULT),
            )));
        }

        let mut source = MockCaptureSource::new(frames, info);
        source.start().unwrap();

        for i in 0..3 {
            let frame = source.poll().unwrap().unwrap();
            match frame {
                MediaFrame::Video(video) => {
                    assert_eq!(video.timestamp.pts_ms(), Some(i as i64));
                    assert_eq!(video.payload.as_ref()[0], i as u8);
                }
                MediaFrame::Audio(_) => panic!("expected video frame"),
            }
        }
        assert!(source.poll().unwrap().is_none());
    }
}

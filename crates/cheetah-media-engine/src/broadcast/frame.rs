//! Bidirectional pipeline frame types.
//!
//! A `MediaFrame` unifies `VideoFrame` and `AudioFrame` so that source,
//! processor and encoder traits can be generic over the media kind.

use alloc::vec::Vec;

use cheetah_media_types::{AudioFrame, BufferRef, MediaTime, VideoFrame};

/// A decoded media frame moving through the broadcast pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaFrame<'a> {
    /// Decoded video frame.
    Video(VideoFrame<'a>),
    /// Decoded audio frame.
    Audio(AudioFrame<'a>),
}

impl<'a> MediaFrame<'a> {
    /// True if this is a video frame.
    pub const fn is_video(&self) -> bool {
        matches!(self, Self::Video(_))
    }

    /// True if this is an audio frame.
    pub const fn is_audio(&self) -> bool {
        matches!(self, Self::Audio(_))
    }

    /// Timestamp attached to the frame.
    pub fn timestamp(&self) -> MediaTime {
        match self {
            Self::Video(f) => f.timestamp,
            Self::Audio(f) => f.timestamp,
        }
    }

    /// Promote borrowed payloads to `'static` shared payloads by copying.
    pub fn to_static(&self) -> MediaFrame<'static> {
        match self {
            Self::Video(f) => MediaFrame::Video(VideoFrame {
                payload: f.payload.to_static(),
                planes: f
                    .planes
                    .iter()
                    .map(BufferRef::to_static)
                    .collect::<Vec<_>>(),
                format: f.format,
                timestamp: f.timestamp,
                handle: f.handle,
            }),
            Self::Audio(f) => MediaFrame::Audio(AudioFrame {
                payload: f.payload.to_static(),
                planes: f
                    .planes
                    .iter()
                    .map(BufferRef::to_static)
                    .collect::<Vec<_>>(),
                format: f.format,
                timestamp: f.timestamp,
                handle: f.handle,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_types::{MediaTime, TimeBase, Timestamp, VideoFormat};

    fn video_format() -> VideoFormat {
        VideoFormat {
            pixel_format: cheetah_media_types::PixelFormat::Rgba,
            coded_width: 64,
            coded_height: 64,
            visible_width: 64,
            visible_height: 64,
            stride: 64 * 4,
            color_space: cheetah_media_types::ColorSpace::Bt709,
        }
    }

    fn make_time(ticks: i64) -> MediaTime {
        let ts = Timestamp::new(ticks);
        MediaTime::from_pts_dts(ts, ts, TimeBase::DEFAULT)
    }

    #[test]
    fn media_frame_kind_and_timestamp() {
        let time = make_time(1000);
        let video = MediaFrame::Video(VideoFrame::new(
            vec![0u8; 64 * 64 * 4],
            video_format(),
            time,
        ));
        assert!(video.is_video());
        assert!(!video.is_audio());
        assert_eq!(video.timestamp().pts_ms(), time.pts_ms());
    }

    #[test]
    fn to_static_promotes_borrowed_payload() {
        let data: Vec<u8> = vec![1, 2, 3];
        let borrowed = MediaFrame::Video(VideoFrame::new(
            data.as_slice(),
            video_format(),
            make_time(0),
        ));
        let owned = borrowed.to_static();
        // Drop original borrow and ensure the static copy still holds the data.
        drop(data);
        assert_eq!(owned.timestamp().pts_ms(), Some(0));
        match owned {
            MediaFrame::Video(f) => assert_eq!(f.payload.as_ref(), &[1, 2, 3]),
            _ => panic!("expected video"),
        }
    }
}

//! Capture source abstraction for the broadcast pipeline.
//!
//! Real platform capture sources (camera, microphone, screen) will be
//! implemented in WP-71. The host-side placeholder returns `MediaError::Unsupported`
//! so the crate can be compiled and tested without platform SDKs.

use cheetah_media_types::MediaError;

use crate::broadcast::frame::MediaFrame;

/// A source that produces raw media frames.
pub trait CaptureSource: Send {
    /// Start capturing. May fail if permissions or hardware are unavailable.
    fn start(&mut self) -> Result<(), MediaError>;

    /// Stop capturing and release any hardware resources.
    fn stop(&mut self) -> Result<(), MediaError>;

    /// Poll the next captured frame, if any.
    ///
    /// Returns `Ok(None)` when no new frame is available but capture is still
    /// running. Returns an error if capture failed.
    fn poll(&mut self) -> Result<Option<MediaFrame<'static>>, MediaError>;

    /// Human-readable source kind (e.g. "camera", "screen", "microphone").
    fn kind(&self) -> &'static str;
}

/// Placeholder capture source used when no platform source is linked.
pub struct UnsupportedCaptureSource;

impl CaptureSource for UnsupportedCaptureSource {
    fn start(&mut self) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7001,
            context: Some("capture source not linked"),
        })
    }

    fn stop(&mut self) -> Result<(), MediaError> {
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<MediaFrame<'static>>, MediaError> {
        Err(MediaError::Unsupported {
            code: 7001,
            context: Some("capture source not linked"),
        })
    }

    fn kind(&self) -> &'static str {
        "unsupported"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_source_rejects_start_and_poll() {
        let mut source = UnsupportedCaptureSource;
        assert!(source.start().is_err());
        assert!(source.poll().is_err());
        assert!(source.stop().is_ok());
        assert_eq!(source.kind(), "unsupported");
    }
}

//! Publisher backend abstraction for the broadcast pipeline.
//!
//! Real network publish paths (WebRTC, RTMP) will be implemented in WP-73. The
//! host-side placeholder returns `MediaError::Unsupported` for publish operations.

use cheetah_media_types::{MediaError, MediaPacket};

/// Network feedback reported by a publisher backend.
///
/// All fields are optional: the backend may only provide a subset depending on
/// the underlying transport (e.g. WebRTC REMB/Transport-CC, RTMP ack).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BitrateFeedback {
    /// Suggested target bitrate in bits per second.
    pub target_bitrate_bps: Option<u32>,
    /// Packet loss fraction scaled to 0-255 (255 = 100% loss).
    pub loss_fraction: Option<u8>,
    /// Round-trip time in milliseconds.
    pub rtt_ms: Option<u32>,
}

/// A backend that sends compressed packets to a network destination.
pub trait PublisherBackend: Send {
    /// Connect to the publish destination identified by `url`.
    fn connect(&mut self, url: &str) -> Result<(), MediaError>;

    /// Publish one packet.
    fn publish(&mut self, packet: &MediaPacket<'static>) -> Result<(), MediaError>;

    /// Flush any buffered packets.
    fn flush(&mut self) -> Result<(), MediaError>;

    /// Poll congestion/transport feedback.
    ///
    /// Returns `None` when no new feedback is available. The pipeline uses this
    /// to drive `Encoder::set_bitrate`.
    fn poll_feedback(&mut self) -> Option<BitrateFeedback>;

    /// Disconnect and release network resources.
    fn disconnect(&mut self);

    /// True if currently connected.
    fn connected(&self) -> bool;

    /// Human-readable backend kind.
    fn kind(&self) -> &'static str;
}

/// Placeholder publisher used when no network backend is linked.
pub struct UnsupportedPublisherBackend;

impl PublisherBackend for UnsupportedPublisherBackend {
    fn connect(&mut self, _url: &str) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7003,
            context: Some("publisher backend not linked"),
        })
    }

    fn publish(&mut self, _packet: &MediaPacket<'static>) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7003,
            context: Some("publisher backend not linked"),
        })
    }

    fn flush(&mut self) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7003,
            context: Some("publisher backend not linked"),
        })
    }

    fn poll_feedback(&mut self) -> Option<BitrateFeedback> {
        None
    }

    fn disconnect(&mut self) {}

    fn connected(&self) -> bool {
        false
    }

    fn kind(&self) -> &'static str {
        "unsupported"
    }
}

/// Placeholder WebRTC publisher backend.
pub struct WebRtcPublisherBackend;

impl PublisherBackend for WebRtcPublisherBackend {
    fn connect(&mut self, _url: &str) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7301,
            context: Some("WebRTC publisher not linked"),
        })
    }

    fn publish(&mut self, _packet: &MediaPacket<'static>) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7301,
            context: Some("WebRTC publisher not linked"),
        })
    }

    fn flush(&mut self) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7301,
            context: Some("WebRTC publisher not linked"),
        })
    }

    fn poll_feedback(&mut self) -> Option<BitrateFeedback> {
        None
    }

    fn disconnect(&mut self) {}

    fn connected(&self) -> bool {
        false
    }

    fn kind(&self) -> &'static str {
        "webrtc"
    }
}

/// Placeholder RTMP publisher backend.
pub struct RtmpPublisherBackend;

impl PublisherBackend for RtmpPublisherBackend {
    fn connect(&mut self, _url: &str) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7302,
            context: Some("RTMP publisher not linked"),
        })
    }

    fn publish(&mut self, _packet: &MediaPacket<'static>) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7302,
            context: Some("RTMP publisher not linked"),
        })
    }

    fn flush(&mut self) -> Result<(), MediaError> {
        Err(MediaError::Unsupported {
            code: 7302,
            context: Some("RTMP publisher not linked"),
        })
    }

    fn poll_feedback(&mut self) -> Option<BitrateFeedback> {
        None
    }

    fn disconnect(&mut self) {}

    fn connected(&self) -> bool {
        false
    }

    fn kind(&self) -> &'static str {
        "rtmp"
    }
}

/// Mock publisher for headless tests; can inject `BitrateFeedback`.
pub struct MockPublisher {
    connected: bool,
    feedback: Option<BitrateFeedback>,
    published: alloc::vec::Vec<MediaPacket<'static>>,
}

impl MockPublisher {
    /// Create a mock publisher with no pending feedback.
    pub fn new() -> Self {
        Self {
            connected: false,
            feedback: None,
            published: alloc::vec::Vec::new(),
        }
    }

    /// Set the feedback to be returned by the next `poll_feedback` call.
    pub fn set_feedback(&mut self, feedback: BitrateFeedback) {
        self.feedback = Some(feedback);
    }

    /// Packets that have been published.
    pub fn published(&self) -> &[MediaPacket<'static>] {
        &self.published
    }
}

impl Default for MockPublisher {
    fn default() -> Self {
        Self::new()
    }
}

impl PublisherBackend for MockPublisher {
    fn connect(&mut self, _url: &str) -> Result<(), MediaError> {
        self.connected = true;
        Ok(())
    }

    fn publish(&mut self, packet: &MediaPacket<'static>) -> Result<(), MediaError> {
        self.published.push(packet.clone());
        Ok(())
    }

    fn flush(&mut self) -> Result<(), MediaError> {
        Ok(())
    }

    fn poll_feedback(&mut self) -> Option<BitrateFeedback> {
        self.feedback.take()
    }

    fn disconnect(&mut self) {
        self.connected = false;
    }

    fn connected(&self) -> bool {
        self.connected
    }

    fn kind(&self) -> &'static str {
        "mock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_publisher_rejects_connect_and_publish() {
        let mut pub_ = UnsupportedPublisherBackend;
        assert!(!pub_.connected());
        assert!(pub_.connect("webrtc://x").is_err());
        assert!(
            pub_.publish(&MediaPacket::new(
                vec![0u8; 4],
                cheetah_media_types::TrackId::new(1).unwrap(),
                cheetah_media_types::StreamEpoch::new(0),
                cheetah_media_types::SequenceNumber::new(0),
                cheetah_media_types::MediaTime::from_pts_dts(
                    cheetah_media_types::Timestamp::new(0),
                    cheetah_media_types::Timestamp::new(0),
                    cheetah_media_types::TimeBase::DEFAULT,
                ),
            ))
            .is_err()
        );
        assert_eq!(pub_.kind(), "unsupported");
    }
}

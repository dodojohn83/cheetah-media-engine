//! Publisher backend abstraction for the broadcast pipeline.
//!
//! Real network publish paths (WebRTC, RTMP) will be implemented in WP-73. The
//! host-side placeholder returns `MediaError::Unsupported` for publish operations.

use cheetah_media_types::{MediaError, MediaPacket};

/// A backend that sends compressed packets to a network destination.
pub trait PublisherBackend: Send {
    /// Connect to the publish destination identified by `url`.
    fn connect(&mut self, url: &str) -> Result<(), MediaError>;

    /// Publish one packet.
    fn publish(&mut self, packet: &MediaPacket<'static>) -> Result<(), MediaError>;

    /// Flush any buffered packets.
    fn flush(&mut self) -> Result<(), MediaError>;

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

    fn disconnect(&mut self) {}

    fn connected(&self) -> bool {
        false
    }

    fn kind(&self) -> &'static str {
        "unsupported"
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

//! Broadcast pipeline: source -> processors -> encoder -> publisher.

use alloc::boxed::Box;
use alloc::vec::Vec;

use cheetah_media_types::{CodecId, MediaError, MediaTime, SequenceNumber, StreamEpoch, TrackId};

use crate::broadcast::encoder::Encoder;
use crate::broadcast::frame::MediaFrame;
use crate::broadcast::processor::Processor;
use crate::broadcast::publisher::PublisherBackend;
use crate::broadcast::source::CaptureSource;
use crate::metrics::Metrics;
use crate::resource::{ResourceKind, ResourceLedger};

/// Immutable configuration for a broadcast pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineConfig {
    /// Track identifier used for emitted packets.
    pub track_id: TrackId,
    /// Stream epoch for emitted packets.
    pub stream_epoch: StreamEpoch,
    /// Target codec.
    pub codec: CodecId,
    /// Coded width.
    pub width: u32,
    /// Coded height.
    pub height: u32,
    /// Target frame rate.
    pub fps: u32,
}

/// Summary returned by `BroadcastPipeline::tick` when a packet is published.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BroadcastPacketSummary {
    /// Packet sequence number within this pipeline.
    pub sequence: u64,
    /// Track identifier.
    pub track_id: TrackId,
    /// Stream epoch of the packet.
    pub stream_epoch: StreamEpoch,
    /// Packet timestamp, if the source frame carried one.
    pub timestamp: Option<MediaTime>,
    /// True if the source frame was audio.
    pub is_audio: bool,
    /// True if the encoded packet is a keyframe.
    pub is_keyframe: bool,
    /// Target bitrate suggested by publisher feedback, if any.
    pub target_bitrate_bps: Option<u32>,
}

/// A one-tick broadcast pipeline.
///
/// The pipeline owns the capture source, processors, encoder, publisher and
/// shared resource/metrics state. Call `tick()` to advance one frame through
/// the entire pipeline.
pub struct BroadcastPipeline {
    source: Box<dyn CaptureSource>,
    processors: Vec<Box<dyn Processor>>,
    encoder: Box<dyn Encoder>,
    publisher: Box<dyn PublisherBackend>,
    metrics: Metrics,
    resources: ResourceLedger,
    config: PipelineConfig,
    sequence: u64,
    started: bool,
    connected: bool,
}

impl BroadcastPipeline {
    /// Current configuration.
    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }

    /// Create a new pipeline.
    pub fn new(
        source: Box<dyn CaptureSource>,
        processors: Vec<Box<dyn Processor>>,
        encoder: Box<dyn Encoder>,
        publisher: Box<dyn PublisherBackend>,
        config: PipelineConfig,
    ) -> Self {
        Self {
            source,
            processors,
            encoder,
            publisher,
            metrics: Metrics::new(),
            resources: ResourceLedger::new(),
            config,
            sequence: 0,
            started: false,
            connected: false,
        }
    }

    /// Connect the publisher to `url`.
    pub fn connect(&mut self, url: &str) -> Result<(), MediaError> {
        self.publisher.connect(url)?;
        if !self.connected {
            self.resources.acquire(ResourceKind::Network);
            self.connected = true;
        }
        Ok(())
    }

    /// Start the capture source and configure the encoder.
    pub fn start(&mut self) -> Result<(), MediaError> {
        if !self.connected {
            return Err(MediaError::InvalidInput {
                code: 7004,
                context: Some("pipeline must be connected before start"),
            });
        }
        self.source.start()?;
        self.encoder.configure(
            self.config.codec,
            self.config.width,
            self.config.height,
            self.config.fps,
        )?;
        self.started = true;
        Ok(())
    }

    /// Stop the pipeline, flush the publisher and release resources.
    pub fn stop(&mut self) -> Result<(), MediaError> {
        let _ = self.source.stop();
        let _ = self.publisher.flush();
        self.publisher.disconnect();
        if self.connected {
            self.resources.release(ResourceKind::Network);
        }
        self.started = false;
        self.connected = false;
        Ok(())
    }

    /// Advance one frame through the pipeline.
    ///
    /// Returns `Ok(None)` if no frame was available, `Ok(Some(summary))` if a
    /// packet was published, or an error if any stage failed.
    pub fn tick(&mut self) -> Result<Option<BroadcastPacketSummary>, MediaError> {
        if !self.started {
            return Ok(None);
        }

        let mut frame = match self.source.poll()? {
            Some(f) => f,
            None => return Ok(None),
        };

        for processor in self.processors.iter_mut() {
            frame = processor.process(&frame)?;
        }

        // Determine the emitted media type from the (possibly transformed) frame.
        let is_audio = matches!(frame, MediaFrame::Audio(_));

        let sequence = SequenceNumber::new(self.sequence);
        let packet = self.encoder.encode(
            &frame,
            self.config.track_id,
            self.config.stream_epoch,
            sequence,
        )?;
        let payload_len = packet.payload.len() as u64;
        let timestamp = if packet.time.has_timestamp() {
            Some(packet.time)
        } else {
            None
        };
        self.publisher.publish(&packet)?;

        let mut target_bitrate_bps = None;
        if let Some(feedback) = self.publisher.poll_feedback()
            && let Some(bps) = feedback.target_bitrate_bps
        {
            self.encoder.set_bitrate(bps)?;
            target_bitrate_bps = Some(bps);
        }

        self.metrics.record_allocation(payload_len);
        let summary = BroadcastPacketSummary {
            sequence: self.sequence,
            track_id: packet.track_id,
            stream_epoch: packet.stream_epoch,
            timestamp,
            is_audio,
            is_keyframe: packet.flags.is_keyframe,
            target_bitrate_bps,
        };
        self.sequence += 1;
        Ok(Some(summary))
    }

    /// Current metrics snapshot.
    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// Mutable access to the encoder.
    pub fn encoder_mut(&mut self) -> &mut dyn Encoder {
        &mut *self.encoder
    }

    /// Current resource ledger.
    pub fn resources(&self) -> &ResourceLedger {
        &self.resources
    }

    /// True if the publisher is connected.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// True if the source has started.
    pub fn is_started(&self) -> bool {
        self.started
    }

    /// Permission required by the capture source, if any.
    pub fn required_permission(&self) -> Option<crate::broadcast::permission::CaptureSourceKind> {
        self.source.required_permission()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broadcast::capture_sources::{MockCaptureSource, VideoFrameInfo};
    use crate::broadcast::encoder::UnsupportedEncoder;
    use crate::broadcast::encoders::MockEncoder;
    use crate::broadcast::publisher::{
        BitrateFeedback, MockPublisher, UnsupportedPublisherBackend,
    };
    use crate::broadcast::source::UnsupportedCaptureSource;
    use cheetah_media_types::{CodecId, ColorSpace, PixelFormat, StreamEpoch, TrackId};

    fn config() -> PipelineConfig {
        PipelineConfig {
            track_id: TrackId::new(1).unwrap(),
            stream_epoch: StreamEpoch::new(0),
            codec: CodecId::H264,
            width: 64,
            height: 64,
            fps: 30,
        }
    }

    fn unsupported_pipeline() -> BroadcastPipeline {
        BroadcastPipeline::new(
            Box::new(UnsupportedCaptureSource),
            Vec::new(),
            Box::new(UnsupportedEncoder),
            Box::new(UnsupportedPublisherBackend),
            config(),
        )
    }

    #[test]
    fn start_requires_connection() {
        let mut pipe = unsupported_pipeline();
        assert!(pipe.start().is_err());
        assert!(!pipe.is_started());
    }

    #[test]
    fn unsupported_publisher_blocks_full_start() {
        let mut pipe = unsupported_pipeline();
        assert!(pipe.connect("webrtc://x").is_err());
        assert!(!pipe.is_connected());
    }

    #[test]
    fn tick_is_noop_before_start() {
        let mut pipe = unsupported_pipeline();
        assert!(pipe.tick().unwrap().is_none());
    }

    #[test]
    fn stop_is_idempotent() {
        let mut pipe = unsupported_pipeline();
        assert!(pipe.stop().is_ok());
        assert!(!pipe.is_started());
    }

    #[test]
    fn repeated_connect_does_not_leak_network_resource() {
        let mut pipe = BroadcastPipeline::new(
            Box::new(UnsupportedCaptureSource),
            Vec::new(),
            Box::new(UnsupportedEncoder),
            Box::new(MockPublisher::new()),
            config(),
        );
        pipe.connect("mock://x").unwrap();
        pipe.connect("mock://x").unwrap();
        assert!(
            pipe.resources()
                .count(crate::resource::ResourceKind::Network)
                > 0
        );
        pipe.stop().unwrap();
        assert_eq!(
            pipe.resources()
                .count(crate::resource::ResourceKind::Network),
            0
        );
    }

    #[test]
    fn tick_applies_publisher_feedback_to_encoder() {
        let mut publisher = MockPublisher::new();
        publisher.set_feedback(BitrateFeedback {
            target_bitrate_bps: Some(500_000),
            loss_fraction: None,
            rtt_ms: None,
        });

        let info = VideoFrameInfo {
            width: 2,
            height: 2,
            stride: 8,
            pixel_format: PixelFormat::Rgba,
            color_space: ColorSpace::Bt709,
        };
        let source = MockCaptureSource::with_count(1, info).unwrap();
        let mut pipe = BroadcastPipeline::new(
            Box::new(source),
            Vec::new(),
            Box::new(MockEncoder::new()),
            Box::new(publisher),
            PipelineConfig {
                track_id: TrackId::new(1).unwrap(),
                stream_epoch: StreamEpoch::new(0),
                codec: CodecId::H264,
                width: 2,
                height: 2,
                fps: 30,
            },
        );

        pipe.connect("mock://x").unwrap();
        pipe.start().unwrap();

        let summary = pipe.tick().unwrap().unwrap();
        assert_eq!(summary.target_bitrate_bps, Some(500_000));
    }
}

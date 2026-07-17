//! Broadcast engine lifecycle state machine.
//!
//! `BroadcastEngine` owns an optional `BroadcastPipeline` and routes commands
//! through an explicit state machine. It shares `ResourceLedger` and `Metrics`
//! with the playback engine.

use alloc::string::String;
use alloc::vec::Vec;

use cheetah_media_types::{MediaError, TrackId};

use crate::broadcast::pipeline::BroadcastPipeline;

/// Lifecycle states for the broadcast engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BroadcastState {
    /// Engine created, no pipeline set.
    #[default]
    Idle,
    /// A pipeline is set but not yet connected.
    Ready,
    /// Attempting to connect the publisher.
    Connecting,
    /// Attempting to start capture and configure the encoder.
    Starting,
    /// Actively capturing, encoding and publishing.
    Broadcasting,
    /// A stop was requested; resources are being released.
    Stopping,
    /// Stopped and can be restarted.
    Stopped,
    /// A non-recoverable failure occurred.
    Failed,
    /// Engine has been destroyed.
    Destroyed,
}

/// Errors that can be returned by `BroadcastEngine::apply`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BroadcastError {
    /// Command is not valid in the current state.
    InvalidState {
        state: BroadcastState,
        command: &'static str,
    },
    /// No pipeline has been attached.
    PipelineNotSet,
    /// Pipeline returned an error.
    Pipeline(MediaError),
}

/// Commands accepted by the broadcast engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BroadcastCommand {
    /// Set the URL for the publisher.
    Connect(String),
    /// Start capture and publishing.
    Start,
    /// Stop capture and publishing.
    Stop,
    /// Request the next encoded frame to be a keyframe.
    RequestKeyframe,
    /// Update the encoder target bitrate.
    SetBitrate(u32),
    /// Destroy the engine and release all resources.
    Destroy,
}

/// Events emitted by the broadcast engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BroadcastEvent {
    /// State changed.
    StateChanged {
        from: BroadcastState,
        to: BroadcastState,
    },
    /// A packet was published.
    PacketPublished { sequence: u64, track_id: TrackId },
    /// An error was raised.
    Error(MediaError),
}

/// Broadcast engine state machine.
pub struct BroadcastEngine {
    state: BroadcastState,
    pipeline: Option<BroadcastPipeline>,
}

impl BroadcastEngine {
    /// Create an engine without a pipeline.
    pub const fn new() -> Self {
        Self {
            state: BroadcastState::Idle,
            pipeline: None,
        }
    }

    /// Create an engine with an attached pipeline.
    pub fn with_pipeline(pipeline: BroadcastPipeline) -> Self {
        Self {
            state: BroadcastState::Ready,
            pipeline: Some(pipeline),
        }
    }

    /// Current state.
    pub const fn state(&self) -> BroadcastState {
        self.state
    }

    /// Apply `command` and return any events emitted.
    pub fn apply(
        &mut self,
        command: BroadcastCommand,
    ) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        match command {
            BroadcastCommand::Connect(url) => self.connect(&url),
            BroadcastCommand::Start => self.start(),
            BroadcastCommand::Stop => self.stop(),
            BroadcastCommand::RequestKeyframe => self.request_keyframe(),
            BroadcastCommand::SetBitrate(bps) => self.set_bitrate(bps),
            BroadcastCommand::Destroy => self.destroy(),
        }
    }

    /// Advance the pipeline by one frame when broadcasting.
    pub fn tick(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if self.state != BroadcastState::Broadcasting {
            return Ok(Vec::new());
        }
        let pipeline = self
            .pipeline
            .as_mut()
            .ok_or(BroadcastError::PipelineNotSet)?;
        match pipeline.tick() {
            Ok(None) => Ok(Vec::new()),
            Ok(Some(summary)) => Ok(vec![BroadcastEvent::PacketPublished {
                sequence: summary.sequence,
                track_id: summary.track_id,
            }]),
            Err(err) => {
                self.state = BroadcastState::Failed;
                Ok(vec![BroadcastEvent::Error(err)])
            }
        }
    }

    fn transition(&mut self, to: BroadcastState) -> Vec<BroadcastEvent> {
        let from = self.state;
        if from == to {
            return Vec::new();
        }
        self.state = to;
        vec![BroadcastEvent::StateChanged { from, to }]
    }

    fn connect(&mut self, url: &str) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if self.state != BroadcastState::Ready && self.state != BroadcastState::Stopped {
            return Err(BroadcastError::InvalidState {
                state: self.state,
                command: "connect",
            });
        }
        let pipeline = self
            .pipeline
            .as_mut()
            .ok_or(BroadcastError::PipelineNotSet)?;
        match pipeline.connect(url) {
            Ok(()) => {
                let mut events = self.transition(BroadcastState::Connecting);
                events.extend(self.transition(BroadcastState::Ready));
                Ok(events)
            }
            Err(err) => {
                self.state = BroadcastState::Failed;
                Ok(vec![BroadcastEvent::Error(err)])
            }
        }
    }

    fn start(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if self.state != BroadcastState::Ready {
            return Err(BroadcastError::InvalidState {
                state: self.state,
                command: "start",
            });
        }
        let pipeline = self
            .pipeline
            .as_mut()
            .ok_or(BroadcastError::PipelineNotSet)?;
        match pipeline.start() {
            Ok(()) => {
                let mut events = self.transition(BroadcastState::Starting);
                events.extend(self.transition(BroadcastState::Broadcasting));
                Ok(events)
            }
            Err(err) => {
                self.state = BroadcastState::Failed;
                Ok(vec![BroadcastEvent::Error(err)])
            }
        }
    }

    fn stop(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if !matches!(
            self.state,
            BroadcastState::Broadcasting
                | BroadcastState::Starting
                | BroadcastState::Ready
                | BroadcastState::Failed
        ) {
            return Err(BroadcastError::InvalidState {
                state: self.state,
                command: "stop",
            });
        }
        if let Some(pipeline) = self.pipeline.as_mut() {
            let _ = pipeline.stop();
        }
        Ok(self.transition(BroadcastState::Stopped))
    }

    fn request_keyframe(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if self.state != BroadcastState::Broadcasting {
            return Err(BroadcastError::InvalidState {
                state: self.state,
                command: "request_keyframe",
            });
        }
        let pipeline = self
            .pipeline
            .as_mut()
            .ok_or(BroadcastError::PipelineNotSet)?;
        pipeline
            .encoder_mut()
            .request_keyframe()
            .map_err(BroadcastError::Pipeline)?;
        Ok(Vec::new())
    }

    fn set_bitrate(&mut self, bps: u32) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if self.state != BroadcastState::Broadcasting {
            return Err(BroadcastError::InvalidState {
                state: self.state,
                command: "set_bitrate",
            });
        }
        let pipeline = self
            .pipeline
            .as_mut()
            .ok_or(BroadcastError::PipelineNotSet)?;
        pipeline
            .encoder_mut()
            .set_bitrate(bps)
            .map_err(BroadcastError::Pipeline)?;
        Ok(Vec::new())
    }

    fn destroy(&mut self) -> Result<Vec<BroadcastEvent>, BroadcastError> {
        if self.state == BroadcastState::Destroyed {
            return Ok(Vec::new());
        }
        if let Some(pipeline) = self.pipeline.as_mut() {
            let _ = pipeline.stop();
        }
        self.pipeline = None;
        Ok(self.transition(BroadcastState::Destroyed))
    }
}

impl Default for BroadcastEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broadcast::encoder::UnsupportedEncoder;
    use crate::broadcast::pipeline::{BroadcastPipeline, PipelineConfig};
    use crate::broadcast::publisher::UnsupportedPublisherBackend;
    use crate::broadcast::source::UnsupportedCaptureSource;
    use cheetah_media_types::{CodecId, StreamEpoch, TrackId};

    fn make_engine() -> BroadcastEngine {
        let config = PipelineConfig {
            track_id: TrackId::new(1).unwrap(),
            stream_epoch: StreamEpoch::new(0),
            codec: CodecId::H264,
            width: 64,
            height: 64,
            fps: 30,
        };
        let pipeline = BroadcastPipeline::new(
            Box::new(UnsupportedCaptureSource),
            Vec::new(),
            Box::new(UnsupportedEncoder),
            Box::new(UnsupportedPublisherBackend),
            config,
        );
        BroadcastEngine::with_pipeline(pipeline)
    }

    #[test]
    fn engine_lifecycle_with_unsupported_pipeline() {
        let mut engine = make_engine();
        assert_eq!(engine.state(), BroadcastState::Ready);

        // Connect fails because the publisher backend is unsupported.
        let events = engine
            .apply(BroadcastCommand::Connect("webrtc://x".into()))
            .unwrap();
        assert_eq!(engine.state(), BroadcastState::Failed);
        assert!(events.iter().any(|e| matches!(e, BroadcastEvent::Error(_))));

        // Stop from Failed should move to Stopped.
        let events = engine.apply(BroadcastCommand::Stop).unwrap();
        assert_eq!(engine.state(), BroadcastState::Stopped);
        assert!(events.iter().any(|e| matches!(
            e,
            BroadcastEvent::StateChanged {
                to: BroadcastState::Stopped,
                ..
            }
        )));

        // Destroy from Stopped is allowed.
        let events = engine.apply(BroadcastCommand::Destroy).unwrap();
        assert_eq!(engine.state(), BroadcastState::Destroyed);
        assert!(events.iter().any(|e| matches!(
            e,
            BroadcastEvent::StateChanged {
                to: BroadcastState::Destroyed,
                ..
            }
        )));
    }

    #[test]
    fn start_requires_ready_state() {
        let mut engine = BroadcastEngine::new();
        assert!(matches!(
            engine.apply(BroadcastCommand::Start).unwrap_err(),
            BroadcastError::InvalidState {
                state: BroadcastState::Idle,
                command: "start"
            }
        ));
    }

    #[test]
    fn request_keyframe_and_set_bitrate_require_broadcasting() {
        let mut engine = make_engine();
        assert!(engine.apply(BroadcastCommand::RequestKeyframe).is_err());
        assert!(
            engine
                .apply(BroadcastCommand::SetBitrate(1_000_000))
                .is_err()
        );
    }
}

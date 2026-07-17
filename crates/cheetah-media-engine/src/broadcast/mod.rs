//! Bidirectional real-time engine abstraction.
//!
//! This module defines the core traits and lifecycle for capture, processing,
//! encoding and publishing media. Platform-specific backends are added in later
//! work packages; WP-70 only provides the trait surface, host-side
//! placeholders and a one-tick pipeline.

pub mod capture_sources;
pub mod encoder;
pub mod encoders;
pub mod engine;
pub mod frame;
pub mod permission;
pub mod pipeline;
pub mod processor;
pub mod publisher;
pub mod registry;
pub mod source;

pub use capture_sources::{
    CameraCaptureSource, MicrophoneCaptureSource, MockCaptureSource, ScreenCaptureSource,
    VideoFrameInfo,
};
pub use encoder::{Encoder, EncoderCapability, UnsupportedEncoder};
pub use encoders::{AacEncoder, G711Encoder, H264Encoder, H265Encoder, MockEncoder, OpusEncoder};
pub use engine::{
    BroadcastCommand, BroadcastDiagnostics, BroadcastEngine, BroadcastError, BroadcastEvent,
    BroadcastState,
};
pub use frame::MediaFrame;
pub use permission::{
    AlwaysDenyPermissionModel, AlwaysGrantPermissionModel, CaptureSourceKind, HostPermissionModel,
    PermissionModel, PermissionState,
};
pub use pipeline::{BroadcastPacketSummary, BroadcastPipeline, PipelineConfig};
pub use processor::{PassThroughProcessor, Processor};
pub use publisher::{
    BitrateFeedback, MockPublisher, PublisherBackend, RtmpPublisherBackend,
    UnsupportedPublisherBackend, WebRtcPublisherBackend,
};
pub use registry::{CaptureSourceRegistry, EncoderRegistry, PublisherBackendRegistry};
pub use source::{CaptureSource, UnsupportedCaptureSource};

//! Platform-neutral backend port definitions.
//!
//! These traits are implemented by platform-specific crates (browser WebCodecs,
//! FFmpeg WASM, native media APIs) and driven by `cheetah-media-engine` from a
//! single serial command loop. All types are `no_std`-compatible and avoid
//! blocking I/O.

use alloc::string::String;

use cheetah_media_abi::Output;
use cheetah_media_types::{CodecId, MediaPacket, StreamEpoch, TrackInfo};

/// Errors reported by a byte transport source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ByteSourceError {
    /// The source has not been started.
    NotStarted,
    /// End of stream reached.
    Eof,
    /// No data is currently available; the caller should poll again later.
    WouldBlock,
    /// A transient failure; the caller may retry after `backoff_ms`.
    Retryable {
        reason: &'static str,
        backoff_ms: u32,
    },
    /// A non-recoverable failure.
    Fatal {
        code: u32,
        context: Option<&'static str>,
    },
    /// The read was cancelled by the caller.
    Cancelled,
}

/// Event returned by a pull/push byte source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ByteSourceEvent<'a> {
    /// A chunk of compressed bytes. The slice borrows the source's internal
    /// buffer and must be consumed or copied before the next call.
    Data(&'a [u8]),
    /// The source is live and producing data, but no bytes are available now.
    Live,
    /// End of stream.
    Eof,
    /// A retryable or fatal error.
    Error(ByteSourceError),
}

/// Statistics for a byte source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceStats {
    /// Total bytes received from the network/container.
    pub bytes_received: u64,
    /// Total bytes delivered to the demuxer.
    pub bytes_consumed: u64,
    /// Whether the source is a live stream (as opposed to a finite file).
    pub is_live: bool,
    /// Estimated download bitrate in bits per second, if known.
    pub bitrate_bps: Option<u64>,
}

/// A byte-oriented transport source (HTTP, HLS, WebSocket, file, etc.).
pub trait ByteSource {
    /// Start fetching `url`. Live sources begin producing `Live` events.
    fn start(&mut self, url: &str) -> Result<(), ByteSourceError>;
    /// Read the next chunk. Returns `Data`, `Live`, `Eof` or an error.
    ///
    /// `buf` may be used as scratch space for pull-style sources; push-style
    /// sources can ignore it and return `Data` referencing an internal buffer.
    fn read_or_push<'a>(&'a mut self, buf: &mut [u8]) -> ByteSourceEvent<'a>;
    /// Cancel any in-flight request and reset the source to a stopped state.
    fn cancel(&mut self) -> Result<(), ByteSourceError>;
    /// Return current source statistics.
    fn stats(&self) -> SourceStats;
}

/// Errors reported by a demuxer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DemuxerError {
    /// The provided bytes are not enough to produce an event yet.
    NeedMore,
    /// The container format is invalid or corrupted.
    InvalidInput,
    /// End of stream.
    Eof,
    /// A non-recoverable failure.
    Fatal {
        code: u32,
        context: Option<&'static str>,
    },
}

/// Event produced by a demuxer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DemuxEvent<'a> {
    /// A new track was discovered.
    Track(TrackInfo),
    /// A compressed packet.
    Packet(MediaPacket<'a>),
    /// A discontinuity; the following packets belong to a new `StreamEpoch`.
    Discontinuity(StreamEpoch),
    /// No complete event could be produced yet.
    NeedMore,
    /// End of stream.
    Eof,
}

/// A container demuxer.
pub trait Demuxer {
    /// Feed `data` into the demuxer and return the next event, if any.
    ///
    /// The returned `Packet` may borrow from the demuxer's internal buffer;
    /// callers must copy or process it before the next `push`.
    fn push<'a>(&'a mut self, data: &[u8]) -> Result<DemuxEvent<'a>, DemuxerError>;
    /// Signal that no more bytes will arrive.
    fn end(&mut self) -> Result<(), DemuxerError>;
    /// Reset the demuxer to a clean state, discarding any buffered data.
    fn reset(&mut self) -> Result<(), DemuxerError>;
}

/// Queue/clock status returned by decoder, renderer and audio sink calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueStatus {
    /// The sample was accepted into the queue.
    Accepted,
    /// The sample was dropped because the queue was over the high watermark.
    Dropped(u64),
    /// Current queue depth in samples/frames.
    QueueDepth(u32),
    /// The operation completed and produced output.
    OutputAvailable,
}

/// Errors reported by a decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecoderError {
    /// The decoder was not configured for this codec.
    NotConfigured,
    /// The input is corrupt or unsupported.
    InvalidInput,
    /// A hardware decoder failed and a fallback should be attempted.
    HardwareFailure,
    /// A non-recoverable failure.
    Fatal {
        code: u32,
        context: Option<&'static str>,
    },
}

/// A compressed sample decoder.
pub trait Decoder {
    /// Configure the decoder for `track`.
    fn configure(&mut self, track: &TrackInfo) -> Result<(), DecoderError>;
    /// Submit a compressed packet.
    fn submit(&mut self, packet: &MediaPacket) -> Result<QueueStatus, DecoderError>;
    /// Flush any buffered frames.
    fn flush(&mut self) -> Result<(), DecoderError>;
    /// Reset the decoder, clearing all state and queued samples.
    fn reset(&mut self) -> Result<(), DecoderError>;
    /// Close and release the decoder.
    fn close(&mut self) -> Result<(), DecoderError>;
}

/// Errors reported by a renderer or audio sink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// Output is not configured.
    NotConfigured,
    /// The sample is too late and was dropped.
    TooLate,
    /// A non-recoverable failure.
    Fatal {
        code: u32,
        context: Option<&'static str>,
    },
}

/// A decoded video renderer.
pub trait Renderer {
    /// Configure the renderer for the current video track.
    fn configure(&mut self, track: &TrackInfo) -> Result<(), RenderError>;
    /// Submit a decoded frame for rendering.
    fn submit(&mut self, frame: &Output) -> Result<QueueStatus, RenderError>;
    /// Set the visible viewport size in pixels.
    fn set_viewport(&mut self, width: u32, height: u32) -> Result<(), RenderError>;
    /// Flush any queued frames.
    fn flush(&mut self) -> Result<(), RenderError>;
    /// Reset the renderer and clear queued frames.
    fn reset(&mut self) -> Result<(), RenderError>;
    /// Close the renderer.
    fn close(&mut self) -> Result<(), RenderError>;
}

/// A decoded audio sink.
pub trait AudioSink {
    /// Configure the sink for the current audio track.
    fn configure(&mut self, track: &TrackInfo) -> Result<(), RenderError>;
    /// Submit decoded audio frames.
    fn submit(&mut self, frame: &Output) -> Result<QueueStatus, RenderError>;
    /// Pause immediately.
    fn pause(&mut self) -> Result<(), RenderError>;
    /// Set the output volume (0.0..=1.0).
    fn set_volume(&mut self, volume: f32) -> Result<(), RenderError>;
    /// Flush any queued frames.
    fn flush(&mut self) -> Result<(), RenderError>;
    /// Reset the sink and clear queued frames.
    fn reset(&mut self) -> Result<(), RenderError>;
    /// Close the sink.
    fn close(&mut self) -> Result<(), RenderError>;
}

/// Errors reported by a recorder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecorderError {
    /// No recording session is active.
    NotStarted,
    /// Write failed.
    WriteFailure,
    /// A non-recoverable failure.
    Fatal {
        code: u32,
        context: Option<&'static str>,
    },
}

/// A media segment recorder.
pub trait Recorder {
    /// Start a new recording at `path`.
    fn start(&mut self, path: &str) -> Result<(), RecorderError>;
    /// Write a compressed sample to the recording.
    fn write(&mut self, packet: &MediaPacket) -> Result<(), RecorderError>;
    /// Stop and finalize the recording.
    fn stop(&mut self) -> Result<(), RecorderError>;
    /// Close the recorder and release resources.
    fn close(&mut self) -> Result<(), RecorderError>;
}

/// A monotonic clock source.
pub trait Clock {
    /// Current media time in milliseconds.
    fn now_ms(&self) -> i64;
}

/// A diagnostics sink for metrics and events.
pub trait MetricsSink {
    /// Report a frame-rendered event.
    fn frame_rendered(&mut self, pts_ms: i64, wall_ms: i64);
    /// Report a decoder/backend error.
    fn decoder_error(&mut self, codec: CodecId, error: String);
    /// Report dropped samples due to queue overrun.
    fn dropped(&mut self, queue: &'static str, count: u64);
    /// Report a backpressure/queue watermark event.
    fn backpressure(&mut self, queue: &'static str, level: u32);
}

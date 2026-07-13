//! Stable ABI and platform-neutral ports for media operations.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

use cheetah_media_types::{CodecId, MediaTime, TrackId};

/// Data carrying a compressed sample and its timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Input<'a> {
    pub data: &'a [u8],
    pub time: MediaTime,
    pub codec: CodecId,
    pub track_id: TrackId,
}

/// A decoded or post-processed media sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Output<'a> {
    pub data: &'a [u8],
    pub time: MediaTime,
    pub duration_ms: u64,
    pub track_id: TrackId,
}

/// Stable ABI error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    NotSupported,
    InvalidData,
    BufferTooSmall,
    WouldBlock,
    Closed,
}

/// Capability probe for a platform decoder.
pub trait DecoderProbe {
    /// True if this decoder can handle `codec`.
    fn supports(&self, codec: CodecId) -> bool;
}

/// Compressed sample decoder.
pub trait Decoder: DecoderProbe {
    /// Feed a compressed sample and return decoded output.
    fn decode<'a>(&mut self, input: &Input<'a>) -> Result<Output<'a>, Error>;
    /// Flush any buffered frames.
    fn flush(&mut self) -> Result<(), Error>;
}

/// Video renderer port.
pub trait Renderer {
    /// Render the decoded frame at the given wallclock time.
    fn render(&mut self, output: &Output) -> Result<(), Error>;
    /// Set the visible viewport size in pixels.
    fn set_viewport(&mut self, width: u32, height: u32) -> Result<(), Error>;
}

/// Audio sink port.
pub trait AudioSink {
    /// Submit decoded audio frames.
    fn play(&mut self, output: &Output) -> Result<(), Error>;
    /// Pause immediately.
    fn pause(&mut self) -> Result<(), Error>;
    /// Set the output volume.
    fn set_volume(&mut self, volume: f32) -> Result<(), Error>;
}

/// Monotonic clock source.
pub trait Clock {
    /// Current media time in milliseconds.
    fn now_ms(&self) -> i64;
}

/// Byte transport source.
pub trait ByteSource {
    /// Read the next chunk of bytes into `buf` and return how many bytes were read.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error>;
    /// Whether the source has reached end-of-stream.
    fn is_eof(&self) -> bool;
}

/// Demuxer that splits a byte stream into timestamped samples.
pub trait Demuxer {
    /// Read the next sample from the transport stream into `buffer` and return its descriptor.
    fn read_sample<'a>(
        &mut self,
        source: &mut dyn ByteSource,
        buffer: &'a mut [u8],
    ) -> Result<Input<'a>, Error>;
    /// Probe whether the container can be parsed.
    fn probe(data: &[u8]) -> bool;
}

/// Recorder sink for storing media segments.
pub trait Recorder {
    /// Start a new recording session.
    fn start(&mut self, path: &str) -> Result<(), Error>;
    /// Write a sample to the recording.
    fn write(&mut self, input: &Input<'_>) -> Result<(), Error>;
    /// Stop and finalize the recording.
    fn stop(&mut self) -> Result<(), Error>;
}

/// Diagnostics sink for metrics and events.
pub trait MetricsSink {
    /// Report a frame-rendered event.
    fn frame_rendered(&mut self, pts_ms: i64, wall_ms: i64);
    /// Report a decoder error.
    fn decoder_error(&mut self, codec: CodecId, error: Error);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSink {
        frame_count: u64,
    }
    impl FakeSink {
        const fn new() -> Self {
            Self { frame_count: 0 }
        }
    }
    impl MetricsSink for FakeSink {
        fn frame_rendered(&mut self, _pts_ms: i64, _wall_ms: i64) {
            self.frame_count += 1;
        }
        fn decoder_error(&mut self, _codec: CodecId, _error: Error) {}
    }

    #[test]
    fn metrics_sink_counts_frames() {
        let mut sink = FakeSink::new();
        sink.frame_rendered(0, 0);
        sink.frame_rendered(0, 0);
        assert_eq!(sink.frame_count, 2);
    }
}

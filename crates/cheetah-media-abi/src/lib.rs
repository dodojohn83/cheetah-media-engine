//! Stable ABI and platform-neutral ports for media operations.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
#[macro_use]
extern crate alloc;

use alloc::vec::Vec;

use cheetah_media_types::{CodecId, MediaTime, TrackId};

pub mod arena;
pub mod descriptor;
pub mod error;
pub mod handle;
pub mod version;

pub use arena::MemoryArena;
pub use descriptor::{FrameDescriptor, MemoryDescriptor, PacketDescriptor};
pub use error::AbiError;
pub use handle::Handle;
pub use version::AbiVersion;

/// Data carrying a compressed sample and its timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Input<'a> {
    pub data: &'a [u8],
    pub time: MediaTime,
    pub codec: CodecId,
    pub track_id: TrackId,
}

/// A decoded or post-processed media sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub data: Vec<u8>,
    pub time: MediaTime,
    pub duration_ms: u64,
    pub track_id: TrackId,
}

/// Capability probe for a platform decoder.
pub trait DecoderProbe {
    /// True if this decoder can handle `codec`.
    fn supports(&self, codec: CodecId) -> bool;
}

/// Compressed sample decoder.
pub trait Decoder: DecoderProbe {
    /// Feed a compressed sample and return decoded output.
    fn decode(&mut self, input: &Input<'_>) -> Result<Output, AbiError>;
    /// Flush any buffered frames.
    fn flush(&mut self) -> Result<(), AbiError>;
}

/// Video renderer port.
pub trait Renderer {
    /// Render the decoded frame at the given wallclock time.
    fn render(&mut self, output: &Output) -> Result<(), AbiError>;
    /// Set the visible viewport size in pixels.
    fn set_viewport(&mut self, width: u32, height: u32) -> Result<(), AbiError>;
}

/// Audio sink port.
pub trait AudioSink {
    /// Submit decoded audio frames.
    fn play(&mut self, output: &Output) -> Result<(), AbiError>;
    /// Pause immediately.
    fn pause(&mut self) -> Result<(), AbiError>;
    /// Set the output volume.
    fn set_volume(&mut self, volume: f32) -> Result<(), AbiError>;
}

/// Monotonic clock source.
pub trait Clock {
    /// Current media time in milliseconds.
    fn now_ms(&self) -> i64;
}

/// Byte transport source.
pub trait ByteSource {
    /// Read the next chunk of bytes into `buf` and return how many bytes were read.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, AbiError>;
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
    ) -> Result<Input<'a>, AbiError>;
    /// Probe whether the container can be parsed.
    fn probe(data: &[u8]) -> bool;
}

/// Recorder sink for storing media segments.
pub trait Recorder {
    /// Start a new recording session.
    fn start(&mut self, path: &str) -> Result<(), AbiError>;
    /// Write a sample to the recording.
    fn write(&mut self, input: &Input<'_>) -> Result<(), AbiError>;
    /// Stop and finalize the recording.
    fn stop(&mut self) -> Result<(), AbiError>;
}

/// Diagnostics sink for metrics and events.
pub trait MetricsSink {
    /// Report a frame-rendered event.
    fn frame_rendered(&mut self, pts_ms: i64, wall_ms: i64);
    /// Report a decoder error.
    fn decoder_error(&mut self, codec: CodecId, error: AbiError);
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
        fn decoder_error(&mut self, _codec: CodecId, _error: AbiError) {}
    }

    #[test]
    fn metrics_sink_counts_frames() {
        let mut sink = FakeSink::new();
        sink.frame_rendered(0, 0);
        sink.frame_rendered(0, 0);
        assert_eq!(sink.frame_count, 2);
    }

    #[test]
    fn abi_version_supports_same_major_and_older_minor() {
        let provider = AbiVersion::new(1, 5);
        let caller = AbiVersion::new(1, 3);
        assert!(provider.supports(caller));
        assert!(!caller.supports(provider));
    }

    #[test]
    fn memory_descriptor_validity() {
        let empty = MemoryDescriptor::empty();
        assert!(!empty.is_valid());
        let desc = MemoryDescriptor {
            region: 0,
            offset: 1024,
            length: 4,
            capacity: 4,
            generation: 1,
            flags: 0,
        };
        assert!(desc.is_valid());
        assert_eq!(desc.range(), Some(1024..1028));
    }

    #[test]
    fn arena_lends_and_reclaims_regions() -> Result<(), AbiError> {
        let mut arena = MemoryArena::new(1);
        let (h, d) = arena.request(8)?;
        assert_eq!(d.length, 8);
        assert_eq!(arena.occupied_count(), 1);
        arena.commit(h, 4)?;
        assert_eq!(arena.read(h)?.len(), 4);
        arena.release(h)?;
        assert_eq!(arena.occupied_count(), 0);
        assert!(arena.read(h).is_err());
        Ok(())
    }

    #[test]
    fn arena_rejects_stale_and_wrong_instance_handles() -> Result<(), AbiError> {
        let mut arena = MemoryArena::new(1);
        let (h, _) = arena.request(8)?;
        arena.release(h)?;
        assert!(matches!(arena.read(h), Err(AbiError::StaleHandle)));

        let bad = Handle {
            instance_id: 2,
            slot: 0,
            generation: 1,
        };
        assert!(matches!(arena.read(bad), Err(AbiError::WrongInstance)));
        Ok(())
    }

    #[test]
    fn arena_detects_double_free() -> Result<(), AbiError> {
        let mut arena = MemoryArena::new(1);
        let (h, _) = arena.request(8)?;
        arena.release(h)?;
        assert!(matches!(arena.release(h), Err(AbiError::DoubleFree)));
        Ok(())
    }

    #[test]
    fn arena_detects_out_of_bounds_commit() -> Result<(), AbiError> {
        let mut arena = MemoryArena::new(1);
        let (h, _) = arena.request(8)?;
        assert!(matches!(arena.commit(h, 16), Err(AbiError::OutOfBounds)));
        Ok(())
    }
}

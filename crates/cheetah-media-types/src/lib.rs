//! Shared media types used across the Cheetah media engine.
//!
//! This crate is `no_std` compatible when the `std` feature is disabled. It depends
//! on `core` and the `alloc` crate for owned containers and `Cow`.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

pub mod buffer;
pub mod error;
pub mod format;
pub mod frame;
pub mod limits;
pub mod metadata;
pub mod packet;
pub mod time;
pub mod track;

pub use buffer::{
    BufferLifecycle, BufferPool, BufferPoolConfig, BufferRef, CopyBudget, CopyCounter, CopyReason,
    DropPolicy, LinearMemoryRef, PoolStats, SimpleBufferPool, StageBudget,
};
pub use error::{MediaError, Recoverability};
pub use format::{AudioFormat, ChannelLayout, ColorSpace, PixelFormat, SampleFormat, VideoFormat};
pub use frame::{AudioFrame, ExternalFrameHandle, VideoFrame};
pub use limits::MediaLimits;
pub use metadata::{MetadataItem, MetadataSource};
pub use packet::{MediaPacket, PacketFlags};
pub use time::{MediaDuration, MediaTime, TimeBase, Timestamp};
pub use track::{CodecConfig, SequenceNumber, StreamEpoch, TrackId, TrackInfo};

/// Identifies a compressed media codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodecId {
    H264,
    H265,
    Aac,
    G711A,
    G711U,
    Mp3,
    Opus,
    PcmU8,
    PcmS16,
    Unknown(u32),
}

/// Whether a track carries video or audio samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackKind {
    Video,
    Audio,
    Data,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_id_round_trip_unknown() {
        let id = CodecId::Unknown(42);
        assert_eq!(id, CodecId::Unknown(42));
    }

    #[test]
    fn track_kind_defaults() {
        let kinds = [TrackKind::Video, TrackKind::Audio, TrackKind::Data];
        assert!(kinds.contains(&TrackKind::Video));
    }
}

//! ISOBMFF / MP4 / fMP4 parser, demuxer, and MSE segmenter.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

pub mod boxes;
pub mod demuxer;
pub mod error;
pub mod fragment;
pub mod moov;
pub mod muxer;
pub mod sample_entry;

pub use demuxer::{IsobmffDemuxer, Mp4Event};
pub use error::Mp4Error;
pub use moov::TrackData;
pub use muxer::{FragmentedMp4Muxer, SegmentOutput, TrackConfig};

#[cfg(test)]
mod tests;

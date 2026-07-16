//! MPEG-2 Program Stream (MPEG-PS) container parser.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

pub mod demuxer;
pub mod error;
pub mod pack;
pub mod pes;
mod scan;

pub use demuxer::{MpegPsConfig, MpegPsDemuxer, MpegPsEvent};
pub use error::MpegPsError;

/// Default maximum accepted PES packet size in bytes.
pub const DEFAULT_MAX_PES_SIZE: usize = 4 * 1024 * 1024;

#[cfg(test)]
mod tests;

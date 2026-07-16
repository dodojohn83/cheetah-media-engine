//! Annex-B H.264/H.265 byte-stream parser.
//!
//! This crate provides an incremental demuxer that splits an Annex-B byte
//! stream into `MediaPacket` events. H.264 is fully supported in this work
//! package; H.265 support will be added in WP-38.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

pub mod demuxer;
pub mod error;
pub mod param_sets;

pub use demuxer::{AnnexBConfig, AnnexBDemuxer, AnnexbEvent};
pub use error::AnnexbError;

#[cfg(test)]
mod tests;

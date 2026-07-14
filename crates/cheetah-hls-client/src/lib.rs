//! HLS / LL-HLS playlist and segment client.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
#[macro_use]
extern crate alloc;

pub mod client;
pub mod error;
pub mod model;
pub mod parser;
pub mod variant;

pub use client::{HlsAction, HlsClient, HlsEvent};
pub use error::HlsError;
pub use model::*;
pub use parser::{parse, parse_master, parse_media};
pub use variant::{VariantCapabilities, VariantSelector, select_initial_variant};

#[cfg(test)]
mod tests;

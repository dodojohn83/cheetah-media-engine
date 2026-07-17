//! Native hardware decoder capability probe and fallback selection.
//!
//! This crate provides:
//! - A platform-neutral `DecoderCapability` model.
//! - `Probe` implementations for Media Foundation, VideoToolbox, VA-API,
//!   Vulkan Video and Software backends.
//! - `CapabilityRegistry` to aggregate probe results and select the best backend.
//! - `NativeDecoder` implementing `cheetah_media_abi::Decoder` with fallback.
//! - A real `G711Decoder` software path for the intercom audio use case.
//!
//! Platform hard-decoder backends are currently stubs that report no
//! capabilities; real OS-specific probing will be added once the target SDKs
//! are linked in the build.

#![cfg_attr(not(feature = "std"), no_std)]
#[macro_use]
extern crate alloc;

pub mod capability;
pub mod decoder;
pub mod g711;
pub mod probe;
pub mod registry;

pub use capability::{BackendKind, DecoderCapability, PlatformApi, VideoCodecSupport};
pub use decoder::NativeDecoder;
pub use g711::G711Decoder;
pub use probe::{
    DefaultProbe, MediaFoundationProbe, Probe, SoftwareProbe, VaApiProbe, VideoToolboxProbe,
    VulkanVideoProbe,
};
pub use registry::CapabilityRegistry;

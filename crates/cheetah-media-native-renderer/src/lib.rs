//! Native renderer capability probe and fallback selection.
//!
//! This crate provides:
//! - A platform-neutral `Surface` / `PixelFormat` model.
//! - `RendererCapability` and `PlatformRenderer` for OpenGL, Vulkan, Metal, D3D11
//!   and Software.
//! - `RendererProbe` implementations for each platform. GPU probes are stubs
//!   until the platform graphics SDKs are linked.
//! - `RendererRegistry` to aggregate capabilities and select the best backend.
//! - `NativeRenderer` implementing `cheetah_media_abi::Renderer` with a CPU
//!   fallback that stores the latest decoded frame in a `Surface`.

#![cfg_attr(not(feature = "std"), no_std)]
#[macro_use]
extern crate alloc;

pub mod capability;
pub mod probe;
pub mod registry;
pub mod renderer;
pub mod surface;

pub use capability::{PixelFormat, PlatformRenderer, RendererCapability, RendererFormatSupport};
pub use probe::{
    D3D11RendererProbe, DefaultRendererProbe, MetalRendererProbe, OpenGlRendererProbe,
    RendererProbe, SoftwareRendererProbe, VulkanRendererProbe,
};
pub use registry::RendererRegistry;
pub use renderer::{
    CpuRenderer, NativeRenderer, RendererBackend, SurfaceAccess, UnsupportedRenderer,
};
pub use surface::{Surface, UploadError};

//! Native audio sink capability probe and A/V sync.
//!
//! This crate provides:
//! - A platform-neutral audio sink capability model (`PlatformAudioSink`,
//!   `AudioSinkCapability`).
//! - `AudioSinkProbe` implementations for ALSA, PulseAudio, CoreAudio, WASAPI
//!   and a `Null` headless sink.
//! - `AudioSinkRegistry` to aggregate capabilities and select the best backend.
//! - `NullAudioSink` implementing `cheetah_media_abi::AudioSink` for CI and
//!   headless environments.
//! - `AvSync` which uses audio as the master clock and decides whether to
//!   render, hold or drop video frames based on drift.

#![cfg_attr(not(feature = "std"), no_std)]
#[macro_use]
extern crate alloc;

pub mod capability;
pub mod probe;
pub mod registry;
pub mod sink;
pub mod sync;

pub use capability::{AudioFormatSupport, AudioSinkCapability, PlatformAudioSink};
pub use probe::{
    AlsaAudioSinkProbe, AudioSinkProbe, CoreAudioSinkProbe, DefaultAudioSinkProbe,
    NullAudioSinkProbe, PulseAudioSinkProbe, WasapiAudioSinkProbe,
};
pub use registry::AudioSinkRegistry;
pub use sink::{NullAudioSink, UnsupportedAudioSink, create_sink};
pub use sync::{AvSync, ManualClock, SyncAction};

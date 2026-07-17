//! Android platform bindings for the Cheetah media engine.
//!
//! This crate exposes Android-specific `Decoder`, `Renderer` and `AudioSink`
//! implementations backed by `MediaCodec`, `Surface` and `AudioTrack`. The
//! non-Android stubs report `Unsupported` so the crate can be compiled and
//! tested on the host; the actual JNI paths are filled in once the Android
//! NDK is linked in the build.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

pub mod audio;
pub mod decoder;
pub mod jni;
pub mod probe;
pub mod renderer;

pub use audio::AndroidAudioSink;
pub use decoder::AndroidDecoder;
pub use jni::{create, destroy, jni_on_load, jni_on_unload};
pub use probe::AndroidMediaCodecProbe;
pub use renderer::AndroidRenderer;

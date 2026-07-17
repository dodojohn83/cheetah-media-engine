//! Audio sink capability description.

use alloc::vec::Vec;
use cheetah_media_types::SampleFormat;

/// Platform audio output API family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformAudioSink {
    /// Linux ALSA.
    Alsa,
    /// Linux/Unix PulseAudio.
    PulseAudio,
    /// Apple CoreAudio.
    CoreAudio,
    /// Windows WASAPI.
    Wasapi,
    /// Null/headless sink for testing.
    Null,
}

/// A supported audio configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFormatSupport {
    pub sample_rate: u32,
    pub channels: u8,
    pub sample_format: SampleFormat,
    pub min_buffer_ms: u32,
    pub max_buffer_ms: u32,
}

/// Capability reported by a single platform audio sink probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioSinkCapability {
    /// Which platform API provides this sink.
    pub api: PlatformAudioSink,
    /// Supported audio configurations.
    pub formats: Vec<AudioFormatSupport>,
    /// Higher values are preferred.
    pub priority: i32,
    /// Whether this sink is expected to work headlessly / in CI.
    pub headless: bool,
}

//! Platform audio sink capability probes.
//!
//! Real platform audio APIs (ALSA, PulseAudio, CoreAudio, WASAPI) are not
//! linked in this cross-platform CI build, so their probes return no
//! capabilities. The `Null` probe reports a headless sink so `AudioSinkRegistry`
//! can always select a working backend for tests.

use alloc::vec::Vec;

use cheetah_media_types::SampleFormat;

use crate::capability::{AudioFormatSupport, AudioSinkCapability, PlatformAudioSink};

/// An audio sink capability probe.
pub trait AudioSinkProbe {
    fn name(&self) -> &'static str;
    fn probe(&self) -> Vec<AudioSinkCapability>;
}

/// Headless null sink probe.
pub struct NullAudioSinkProbe;

impl AudioSinkProbe for NullAudioSinkProbe {
    fn name(&self) -> &'static str {
        "null-audio-sink"
    }

    fn probe(&self) -> Vec<AudioSinkCapability> {
        vec![AudioSinkCapability {
            api: PlatformAudioSink::Null,
            formats: vec![
                AudioFormatSupport {
                    sample_rate: 48000,
                    channels: 2,
                    sample_format: SampleFormat::S16,
                    min_buffer_ms: 0,
                    max_buffer_ms: 1000,
                },
                AudioFormatSupport {
                    sample_rate: 44100,
                    channels: 2,
                    sample_format: SampleFormat::S16,
                    min_buffer_ms: 0,
                    max_buffer_ms: 1000,
                },
            ],
            priority: 0,
            headless: true,
        }]
    }
}

/// ALSA probe (stub).
pub struct AlsaAudioSinkProbe;

impl AudioSinkProbe for AlsaAudioSinkProbe {
    fn name(&self) -> &'static str {
        "alsa"
    }

    fn probe(&self) -> Vec<AudioSinkCapability> {
        Vec::new()
    }
}

/// PulseAudio probe (stub).
pub struct PulseAudioSinkProbe;

impl AudioSinkProbe for PulseAudioSinkProbe {
    fn name(&self) -> &'static str {
        "pulseaudio"
    }

    fn probe(&self) -> Vec<AudioSinkCapability> {
        Vec::new()
    }
}

/// CoreAudio probe (stub).
pub struct CoreAudioSinkProbe;

impl AudioSinkProbe for CoreAudioSinkProbe {
    fn name(&self) -> &'static str {
        "coreaudio"
    }

    fn probe(&self) -> Vec<AudioSinkCapability> {
        Vec::new()
    }
}

/// WASAPI probe (stub).
pub struct WasapiAudioSinkProbe;

impl AudioSinkProbe for WasapiAudioSinkProbe {
    fn name(&self) -> &'static str {
        "wasapi"
    }

    fn probe(&self) -> Vec<AudioSinkCapability> {
        Vec::new()
    }
}

/// Convenience probe that aggregates all built-in audio sink probes.
pub struct DefaultAudioSinkProbe;

impl AudioSinkProbe for DefaultAudioSinkProbe {
    fn name(&self) -> &'static str {
        "default"
    }

    fn probe(&self) -> Vec<AudioSinkCapability> {
        let mut caps = Vec::new();
        caps.extend(AlsaAudioSinkProbe.probe());
        caps.extend(PulseAudioSinkProbe.probe());
        caps.extend(CoreAudioSinkProbe.probe());
        caps.extend(WasapiAudioSinkProbe.probe());
        caps.extend(NullAudioSinkProbe.probe());
        caps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_probe_reports_headless_s16_stereo() {
        let cap = NullAudioSinkProbe.probe().pop().unwrap();
        assert_eq!(cap.api, PlatformAudioSink::Null);
        assert!(cap.headless);
        assert!(cap.formats.iter().any(|f| f.sample_rate == 48000));
    }

    #[test]
    fn default_probe_contains_null() {
        let caps = DefaultAudioSinkProbe.probe();
        assert!(caps.iter().any(|c| c.api == PlatformAudioSink::Null));
    }
}

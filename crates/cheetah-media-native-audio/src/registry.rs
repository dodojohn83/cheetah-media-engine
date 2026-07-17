//! Audio sink capability registry.

use alloc::vec::Vec;

use cheetah_media_types::AudioFormat;

use crate::capability::{AudioSinkCapability, PlatformAudioSink};
use crate::probe::AudioSinkProbe;

/// Aggregated audio sink capabilities from all platform probes.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AudioSinkRegistry {
    entries: Vec<AudioSinkCapability>,
}

impl AudioSinkRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Create a registry pre-populated from a single probe.
    pub fn with_probe<P: AudioSinkProbe>(probe: P) -> Self {
        let mut reg = Self::new();
        reg.register(probe);
        reg
    }

    /// Add a manually-specified capability.
    pub fn add(&mut self, cap: AudioSinkCapability) {
        self.entries.push(cap);
    }

    /// Run a probe and register its capabilities.
    pub fn register<P: AudioSinkProbe>(&mut self, probe: P) {
        self.entries.extend(probe.probe());
    }

    /// Select the best audio sink for a given audio format.
    /// Returns `None` if no registered sink reports support.
    pub fn select(&self, format: &AudioFormat) -> Option<PlatformAudioSink> {
        let mut candidates: Vec<_> = self
            .entries
            .iter()
            .filter(|cap| supports_format(cap, format))
            .collect();
        candidates.sort_by_key(|cap| -cap.priority);
        candidates.first().map(|cap| cap.api)
    }

    /// Return all registered capabilities.
    pub fn capabilities(&self) -> &[AudioSinkCapability] {
        &self.entries
    }
}

fn supports_format(cap: &AudioSinkCapability, format: &AudioFormat) -> bool {
    cap.formats.iter().any(|f| {
        f.sample_rate == format.sample_rate
            && f.channels == format.channel_layout.channels() as u8
            && f.sample_format == format.sample_format
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_types::{ChannelLayout, SampleFormat};

    fn s48_stereo() -> AudioFormat {
        AudioFormat {
            sample_format: SampleFormat::S16,
            sample_rate: 48000,
            channel_layout: ChannelLayout::Stereo,
            sample_count: 0,
        }
    }

    #[test]
    fn select_prefers_higher_priority() {
        let mut reg = AudioSinkRegistry::new();
        reg.add(AudioSinkCapability {
            api: PlatformAudioSink::Null,
            formats: vec![crate::capability::AudioFormatSupport {
                sample_rate: 48000,
                channels: 2,
                sample_format: SampleFormat::S16,
                min_buffer_ms: 0,
                max_buffer_ms: 1000,
            }],
            priority: 0,
            headless: true,
        });
        reg.add(AudioSinkCapability {
            api: PlatformAudioSink::Wasapi,
            formats: vec![crate::capability::AudioFormatSupport {
                sample_rate: 48000,
                channels: 2,
                sample_format: SampleFormat::S16,
                min_buffer_ms: 0,
                max_buffer_ms: 1000,
            }],
            priority: 10,
            headless: false,
        });
        assert_eq!(reg.select(&s48_stereo()), Some(PlatformAudioSink::Wasapi));
    }

    #[test]
    fn select_respects_sample_rate() {
        let mut reg = AudioSinkRegistry::new();
        reg.add(AudioSinkCapability {
            api: PlatformAudioSink::Null,
            formats: vec![crate::capability::AudioFormatSupport {
                sample_rate: 44100,
                channels: 2,
                sample_format: SampleFormat::S16,
                min_buffer_ms: 0,
                max_buffer_ms: 1000,
            }],
            priority: 0,
            headless: true,
        });
        assert_eq!(reg.select(&s48_stereo()), None);
    }
}

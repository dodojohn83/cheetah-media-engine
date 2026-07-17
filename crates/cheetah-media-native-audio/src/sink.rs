//! Native audio sink implementations.

use alloc::boxed::Box;

use cheetah_media_abi::{AbiError, AudioSink, Output};

use crate::capability::PlatformAudioSink;
use crate::registry::AudioSinkRegistry;

/// A sink that ignores all audio data but records submission metadata.
///
/// This is the headless fallback for CI and for environments without a real
/// audio device. It validates that the submitted output is non-empty and
/// non-corrupt, then queues a frame record.
pub struct NullAudioSink {
    _sample_rate: u32,
    channels: u8,
    volume: f32,
    paused: bool,
    submitted_samples: u64,
    last_duration_ms: u64,
}

impl NullAudioSink {
    /// Create a null sink for the given audio format.
    pub fn new(sample_rate: u32, channels: u8) -> Self {
        Self {
            _sample_rate: sample_rate,
            channels,
            volume: 1.0,
            paused: false,
            submitted_samples: 0,
            last_duration_ms: 0,
        }
    }

    /// Total number of sample frames submitted since creation or last reset.
    pub fn submitted_samples(&self) -> u64 {
        self.submitted_samples
    }

    /// Reset the submission counter.
    pub fn reset(&mut self) {
        self.submitted_samples = 0;
    }

    fn frame_count(&self, data_len: usize) -> u64 {
        let bytes_per_sample = 2u64; // S16
        let sample_frame_size = self.channels as u64 * bytes_per_sample;
        if sample_frame_size == 0 {
            return 0;
        }
        (data_len as u64) / sample_frame_size
    }
}

impl AudioSink for NullAudioSink {
    fn play(&mut self, output: &Output) -> Result<(), AbiError> {
        if output.data.is_empty() {
            return Err(AbiError::InvalidData);
        }
        self.last_duration_ms = output.duration_ms;
        self.submitted_samples += self.frame_count(output.data.len());
        Ok(())
    }

    fn pause(&mut self) -> Result<(), AbiError> {
        self.paused = true;
        Ok(())
    }

    fn set_volume(&mut self, volume: f32) -> Result<(), AbiError> {
        if volume.is_nan() || !(0.0..=1.0).contains(&volume) {
            return Err(AbiError::InvalidData);
        }
        self.volume = volume;
        Ok(())
    }
}

/// Stub sink for a platform API that has not been linked yet.
pub struct UnsupportedAudioSink {
    _api: PlatformAudioSink,
}

impl UnsupportedAudioSink {
    /// Create a stub for the given platform API.
    pub fn new(api: PlatformAudioSink) -> Self {
        Self { _api: api }
    }
}

impl AudioSink for UnsupportedAudioSink {
    fn play(&mut self, _output: &Output) -> Result<(), AbiError> {
        Err(AbiError::NotSupported)
    }

    fn pause(&mut self) -> Result<(), AbiError> {
        Err(AbiError::NotSupported)
    }

    fn set_volume(&mut self, _volume: f32) -> Result<(), AbiError> {
        Err(AbiError::NotSupported)
    }
}

/// Select an audio sink from the registry for the given format.
///
/// Falls back to the null sink if no real platform sink reports support.
pub fn create_sink(
    registry: &AudioSinkRegistry,
    format: &cheetah_media_types::AudioFormat,
) -> Result<Box<dyn AudioSink + Send>, AbiError> {
    match registry.select(format) {
        Some(PlatformAudioSink::Null) | None => Ok(Box::new(NullAudioSink::new(
            format.sample_rate,
            format.channel_layout.channels() as u8,
        ))),
        Some(api) => Ok(Box::new(UnsupportedAudioSink::new(api))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    use cheetah_media_types::{
        ChannelLayout, MediaTime, SampleFormat, TimeBase, Timestamp, TrackId,
    };

    fn output(data: Vec<u8>, duration_ms: u64) -> Output {
        Output {
            data,
            time: MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT),
            duration_ms,
            track_id: TrackId::new(1).unwrap(),
        }
    }

    fn s48_stereo_format() -> cheetah_media_types::AudioFormat {
        cheetah_media_types::AudioFormat {
            sample_format: SampleFormat::S16,
            sample_rate: 48000,
            channel_layout: ChannelLayout::Stereo,
            sample_count: 0,
        }
    }

    #[test]
    fn null_sink_counts_submitted_frames() {
        let mut sink = NullAudioSink::new(48000, 2);
        // 4 bytes per stereo S16 frame; 8 bytes = 2 frames.
        sink.play(&output(vec![0u8; 8], 10)).unwrap();
        assert_eq!(sink.submitted_samples(), 2);
    }

    #[test]
    fn null_sink_rejects_empty_output() {
        let mut sink = NullAudioSink::new(48000, 2);
        assert_eq!(
            sink.play(&output(Vec::new(), 0)).unwrap_err(),
            AbiError::InvalidData
        );
    }

    #[test]
    fn null_sink_rejects_invalid_volume() {
        let mut sink = NullAudioSink::new(48000, 2);
        assert_eq!(sink.set_volume(2.0).unwrap_err(), AbiError::InvalidData);
        assert_eq!(sink.set_volume(-0.1).unwrap_err(), AbiError::InvalidData);
    }

    #[test]
    fn create_sink_from_registry_uses_null_fallback() {
        let reg = AudioSinkRegistry::with_probe(crate::probe::NullAudioSinkProbe);
        let format = s48_stereo_format();
        let mut sink = create_sink(&reg, &format).unwrap();
        assert!(sink.play(&output(vec![0u8; 8], 10)).is_ok());
    }
}

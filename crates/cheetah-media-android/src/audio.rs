//! Android `AudioTrack` sink implementation.
//!
//! Host-side stub: playback requires an Android `AudioTrack` instance and will
//! be wired in WP-64.

use cheetah_media_abi::{AbiError, AudioSink, Output};

/// Audio sink backed by Android `AudioTrack`.
pub struct AndroidAudioSink;

impl AndroidAudioSink {
    /// Create a new Android audio sink.
    pub const fn new() -> Self {
        Self
    }
}

impl Default for AndroidAudioSink {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioSink for AndroidAudioSink {
    fn play(&mut self, _output: &Output) -> Result<(), AbiError> {
        Err(AbiError::NotSupported)
    }

    fn pause(&mut self) -> Result<(), AbiError> {
        Ok(())
    }

    fn set_volume(&mut self, _volume: f32) -> Result<(), AbiError> {
        Err(AbiError::NotSupported)
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;
    use cheetah_media_abi::Output;
    use cheetah_media_types::{MediaTime, TimeBase, Timestamp, TrackId};

    fn empty_output() -> Output {
        Output {
            data: Vec::new(),
            time: MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT),
            duration_ms: 0,
            track_id: TrackId::new(1).unwrap(),
        }
    }

    #[test]
    fn host_stub_rejects_play_and_set_volume() {
        let mut sink = AndroidAudioSink::new();
        assert_eq!(
            sink.play(&empty_output()).unwrap_err(),
            AbiError::NotSupported
        );
        assert_eq!(sink.set_volume(0.5).unwrap_err(), AbiError::NotSupported);
    }

    #[test]
    fn pause_is_allowed_without_playback() {
        let mut sink = AndroidAudioSink::new();
        assert!(sink.pause().is_ok());
    }
}

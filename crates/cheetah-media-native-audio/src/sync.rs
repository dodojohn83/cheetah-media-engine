//! Audio/video synchronization.

use cheetah_media_abi::{AbiError, Clock, Output, Renderer};

use cheetah_media_abi::AudioSink;

use crate::sink::NullAudioSink;

/// Action returned by `AvSync` after submitting a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncAction {
    /// Render/play immediately.
    Render,
    /// Hold until the given wallclock time (milliseconds).
    HoldUntil(i64),
    /// Drop the frame because it is too late.
    Drop,
}

/// Synchronizes audio and video presentation.
///
/// Audio is treated as the master clock. Video frames are delayed or dropped
/// to keep them within `max_video_drift_ms` of the audio clock.
pub struct AvSync<A: AudioSink, C: Clock, R: Renderer> {
    audio: A,
    renderer: R,
    clock: C,
    /// Maximum acceptable video lead/lag in milliseconds.
    max_video_drift_ms: i64,
    /// Audio rendering latency budget in milliseconds.
    audio_latency_ms: i64,
    /// Accumulated audio duration in milliseconds (master clock).
    audio_clock_ms: i64,
    /// Last submitted video pts in milliseconds.
    last_video_pts_ms: i64,
}

impl<A: AudioSink, C: Clock, R: Renderer> AvSync<A, C, R> {
    /// Create an A/V sync controller with the given audio latency budget and drift tolerance.
    pub fn new(
        audio: A,
        renderer: R,
        clock: C,
        max_video_drift_ms: i64,
        audio_latency_ms: i64,
    ) -> Self {
        Self {
            audio,
            renderer,
            clock,
            max_video_drift_ms,
            audio_latency_ms,
            audio_clock_ms: 0,
            last_video_pts_ms: 0,
        }
    }

    /// Submit an audio frame to the sink and advance the master clock.
    pub fn submit_audio(&mut self, output: &Output) -> Result<SyncAction, AbiError> {
        self.audio.play(output)?;
        let duration = i64::try_from(output.duration_ms).unwrap_or(i64::MAX);
        self.audio_clock_ms = self.audio_clock_ms.saturating_add(duration);
        Ok(SyncAction::Render)
    }

    /// Submit a video frame and return the synchronization action.
    pub fn submit_video(&mut self, output: &Output) -> Result<SyncAction, AbiError> {
        let pts_ms = output.time.pts_ms().unwrap_or(0);
        self.last_video_pts_ms = pts_ms;

        let now_ms = self.clock.now_ms();
        let target_pts_ms = self.audio_clock_ms.saturating_add(self.audio_latency_ms);
        let drift_ms = pts_ms.saturating_sub(target_pts_ms);

        if drift_ms < 0 && drift_ms.saturating_add(self.max_video_drift_ms) < 0 {
            return Ok(SyncAction::Drop);
        }

        if drift_ms > 0 && drift_ms.saturating_sub(self.max_video_drift_ms) > 0 {
            let hold_until = now_ms.saturating_add(drift_ms);
            return Ok(SyncAction::HoldUntil(hold_until));
        }

        self.renderer.render(output)?;
        Ok(SyncAction::Render)
    }

    /// Pause both audio and video paths.
    pub fn pause(&mut self) -> Result<(), AbiError> {
        self.audio.pause()?;
        Ok(())
    }

    /// Set audio volume.
    pub fn set_volume(&mut self, volume: f32) -> Result<(), AbiError> {
        self.audio.set_volume(volume)?;
        Ok(())
    }

    /// Access the audio sink.
    pub fn audio(&self) -> &A {
        &self.audio
    }

    /// Access the renderer.
    pub fn renderer(&self) -> &R {
        &self.renderer
    }
}

impl<C: Clock, R: Renderer> AvSync<NullAudioSink, C, R> {
    /// Convenience constructor for a headless A/V sync test harness.
    pub fn headless(renderer: R, clock: C, max_video_drift_ms: i64) -> Self {
        use cheetah_media_types::SampleFormat;
        Self::new(
            NullAudioSink::new(48000, 2, SampleFormat::S16),
            renderer,
            clock,
            max_video_drift_ms,
            0,
        )
    }
}

/// A simple test clock that can be advanced manually.
#[derive(Debug, Clone, Copy, Default)]
pub struct ManualClock {
    now_ms: i64,
}

impl ManualClock {
    /// Create a clock starting at 0.
    pub fn new() -> Self {
        Self { now_ms: 0 }
    }

    /// Advance the clock by `delta_ms`.
    pub fn advance(&mut self, delta_ms: i64) {
        self.now_ms = self.now_ms.saturating_add(delta_ms);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> i64 {
        self.now_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_abi::{Output, Renderer};
    use cheetah_media_types::{MediaTime, TimeBase, Timestamp, TrackId};

    struct DummyRenderer {
        rendered: u32,
    }

    impl DummyRenderer {
        fn new() -> Self {
            Self { rendered: 0 }
        }
    }

    impl Renderer for DummyRenderer {
        fn render(&mut self, _output: &Output) -> Result<(), AbiError> {
            self.rendered += 1;
            Ok(())
        }

        fn set_viewport(&mut self, _width: u32, _height: u32) -> Result<(), AbiError> {
            Ok(())
        }
    }

    fn video_output(pts_ms: i64) -> Output {
        Output {
            data: vec![0u8; 16],
            time: MediaTime::from_pts_dts(
                Timestamp::new(pts_ms),
                Timestamp::new(pts_ms),
                TimeBase::DEFAULT,
            ),
            duration_ms: 33,
            track_id: TrackId::new(1).unwrap(),
        }
    }

    fn audio_output(duration_ms: u64) -> Output {
        Output {
            data: vec![0u8; 8],
            time: MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT),
            duration_ms,
            track_id: TrackId::new(2).unwrap(),
        }
    }

    #[test]
    fn video_on_time_is_rendered() {
        let renderer = DummyRenderer::new();
        let clock = ManualClock::new();
        let mut sync = AvSync::headless(renderer, clock, 60);

        sync.submit_audio(&audio_output(0)).unwrap();
        let action = sync.submit_video(&video_output(0)).unwrap();
        assert_eq!(action, SyncAction::Render);
        assert_eq!(sync.renderer().rendered, 1);
    }

    #[test]
    fn video_too_late_is_dropped() {
        let renderer = DummyRenderer::new();
        let clock = ManualClock::new();
        let mut sync = AvSync::headless(renderer, clock, 60);

        // Audio clock at 100ms; video at 0ms is too late.
        sync.submit_audio(&audio_output(100)).unwrap();
        let action = sync.submit_video(&video_output(0)).unwrap();
        assert_eq!(action, SyncAction::Drop);
        assert_eq!(sync.renderer().rendered, 0);
    }

    #[test]
    fn video_too_early_is_held() {
        let renderer = DummyRenderer::new();
        let clock = ManualClock::new();
        let mut sync = AvSync::headless(renderer, clock, 60);

        // Audio clock at 0ms; video at 100ms is too early.
        sync.submit_audio(&audio_output(0)).unwrap();
        let action = sync.submit_video(&video_output(100)).unwrap();
        assert!(matches!(action, SyncAction::HoldUntil(_)));
        assert_eq!(sync.renderer().rendered, 0);
    }

    #[test]
    fn pause_pauses_audio_sink() {
        let renderer = DummyRenderer::new();
        let clock = ManualClock::new();
        let mut sync = AvSync::headless(renderer, clock, 60);
        assert!(sync.pause().is_ok());
    }
}

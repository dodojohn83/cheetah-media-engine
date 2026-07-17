//! Native player orchestration.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use cheetah_media_abi::{AbiError, AudioSink, Decoder, Input, Output, Renderer};
use cheetah_media_backend_api::{ByteSource, ByteSourceError, ByteSourceEvent};
use cheetah_media_native_audio::{AudioSinkRegistry, NullAudioSinkProbe, create_sink};
use cheetah_media_native_decoder::{CapabilityRegistry, DefaultProbe, NativeDecoder};
use cheetah_media_native_renderer::{DefaultRendererProbe, NativeRenderer, RendererRegistry};
use cheetah_media_native_transport::NativeByteSource;
use cheetah_media_types::{MediaTime, TimeBase, Timestamp, TrackInfo, TrackKind};

use crate::native::diagnostics::{DiagnosticEvent, Diagnostics};
use crate::native::lifecycle::{LifecycleError, LifecycleEvent, LifecycleSoak};
use crate::native::negotiator::{AudioTarget, BackendPlan, NegotiationError, VideoTarget};
use crate::native::source::MemoryByteSource;
use crate::state::{
    BackendEvent, Engine, EngineCommand, EngineError, EngineEvent, LoadRequest, NetworkEvent,
    PlayerState,
};

/// Configuration for a native player session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePlayerConfig {
    /// URL or identifier of the source.
    pub url: String,
    /// Static description of the single track this player will consume.
    pub track: TrackInfo,
    /// Optional video output target.
    pub video: Option<VideoTarget>,
    /// Optional audio output target.
    pub audio: Option<AudioTarget>,
    /// Whether to begin playback immediately after `load`.
    pub autoplay: bool,
}

/// Errors reported by the native player.
#[derive(Debug)]
pub enum NativePlayerError {
    Engine(EngineError),
    Source(ByteSourceError),
    Abi(AbiError),
    Negotiation(NegotiationError),
    Lifecycle(LifecycleError),
    MissingSource,
}

impl From<EngineError> for NativePlayerError {
    fn from(e: EngineError) -> Self {
        Self::Engine(e)
    }
}

impl From<ByteSourceError> for NativePlayerError {
    fn from(e: ByteSourceError) -> Self {
        Self::Source(e)
    }
}

impl From<AbiError> for NativePlayerError {
    fn from(e: AbiError) -> Self {
        Self::Abi(e)
    }
}

impl From<NegotiationError> for NativePlayerError {
    fn from(e: NegotiationError) -> Self {
        Self::Negotiation(e)
    }
}

impl From<LifecycleError> for NativePlayerError {
    fn from(e: LifecycleError) -> Self {
        Self::Lifecycle(e)
    }
}

/// A native media player that wires a byte source, decoder and output sink
/// through the engine state machine.
pub struct NativePlayer {
    engine: Engine,
    source: Box<dyn ByteSource>,
    track: TrackInfo,
    _video: Option<VideoTarget>,
    audio: Option<AudioTarget>,
    decoder: Box<dyn Decoder>,
    renderer: Option<Box<dyn Renderer>>,
    audio_sink: Option<Box<dyn AudioSink + Send>>,
    diagnostics: Diagnostics,
    lifecycle: LifecycleSoak,
    scratch: [u8; 8192],
}

impl NativePlayer {
    /// Create a player from an already-negotiated plan and components.
    ///
    /// Prefer `NativePlayerBuilder` for most use cases.
    pub fn new(
        source: Box<dyn ByteSource>,
        track: TrackInfo,
        video: Option<VideoTarget>,
        audio: Option<AudioTarget>,
        decoder: Box<dyn Decoder>,
        renderer: Option<Box<dyn Renderer>>,
        audio_sink: Option<Box<dyn AudioSink + Send>>,
    ) -> Self {
        let mut lifecycle = LifecycleSoak::new();
        lifecycle.record(LifecycleEvent::Created);
        Self {
            engine: Engine::new(),
            source,
            track,
            _video: video,
            audio,
            decoder,
            renderer,
            audio_sink,
            diagnostics: Diagnostics::default(),
            lifecycle,
            scratch: [0u8; 8192],
        }
    }

    /// Load `url` and transition through `Loading` to `Preroll`.
    pub fn load(&mut self, url: &str) -> Result<Vec<EngineEvent>, NativePlayerError> {
        self.source.start(url)?;
        let before = self.engine.state();
        let mut out = self
            .engine
            .apply(EngineCommand::Load(LoadRequest {
                url: url.into(),
                is_live: false,
            }))?
            .events;

        let epoch = self.engine.epoch();
        out.extend(
            self.engine
                .apply(EngineCommand::Backend(BackendEvent::Track {
                    epoch,
                    info: self.track.clone(),
                }))?
                .events,
        );
        out.extend(
            self.engine
                .apply(EngineCommand::Backend(BackendEvent::ConfigChanged {
                    epoch,
                    track_id: self.track.id,
                    generation: self.track.generation,
                }))?
                .events,
        );

        if self.engine.state() != before {
            self.lifecycle
                .record(LifecycleEvent::Loaded { url: url.into() });
        }

        if self.engine.state() == PlayerState::Preroll {
            self.lifecycle.record(LifecycleEvent::Prerolled);
            self.diagnostics.backend_selected("decoder", "native");
            if self.renderer.is_some() {
                self.diagnostics.backend_selected("renderer", "native");
            }
            if self.audio_sink.is_some() {
                self.diagnostics.backend_selected("audio", "null");
            }
        }

        Ok(out)
    }

    /// Begin playback.
    pub fn play(&mut self) -> Result<Vec<EngineEvent>, NativePlayerError> {
        let before = self.engine.state();
        let out = self.engine.apply(EngineCommand::Play)?;
        if self.engine.state() != before {
            self.lifecycle.record(LifecycleEvent::Played);
        }
        Ok(out.events)
    }

    /// Pause playback.
    pub fn pause(&mut self) -> Result<Vec<EngineEvent>, NativePlayerError> {
        let before = self.engine.state();
        let out = self.engine.apply(EngineCommand::Pause)?;
        if self.engine.state() != before {
            self.lifecycle.record(LifecycleEvent::Paused);
        }
        Ok(out.events)
    }

    /// Stop and release the current session.
    pub fn stop(&mut self) -> Result<Vec<EngineEvent>, NativePlayerError> {
        let before = self.engine.state();
        let out = self.engine.apply(EngineCommand::Stop)?;
        if self.engine.state() != before {
            self.lifecycle.record(LifecycleEvent::Stopped);
        }
        let _ = self.decoder.flush();
        self.source.cancel()?;
        Ok(out.events)
    }

    /// Tear down the player and validate its lifecycle.
    pub fn destroy(mut self) -> Result<Vec<EngineEvent>, NativePlayerError> {
        let out = self.engine.apply(EngineCommand::Destroy)?;
        if !self.lifecycle.is_destroyed() {
            self.lifecycle.record(LifecycleEvent::Destroyed);
        }
        self.lifecycle.validate()?;
        Ok(out.events)
    }

    /// Drive one decoding/rendering tick. Call from a loop while the player is
    /// `Playing`.
    pub fn tick(&mut self) -> Result<Vec<EngineEvent>, NativePlayerError> {
        if self.engine.state() != PlayerState::Playing {
            return Ok(Vec::new());
        }

        let mut events = Vec::new();
        let data_opt: Option<Vec<u8>> = {
            let event = self.source.read_or_push(&mut self.scratch);
            match event {
                ByteSourceEvent::Data(chunk) => Some(chunk.to_vec()),
                ByteSourceEvent::Eof => {
                    events.extend(
                        self.engine
                            .apply(EngineCommand::Network(NetworkEvent::Eof))?
                            .events,
                    );
                    None
                }
                ByteSourceEvent::Error(e) => {
                    self.diagnostics.record(DiagnosticEvent::Error {
                        stage: "source",
                        code: byte_source_error_code(&e),
                    });
                    match e {
                        ByteSourceError::Retryable { .. } => {
                            events.extend(
                                self.engine
                                    .apply(EngineCommand::Network(NetworkEvent::Retryable))?
                                    .events,
                            );
                        }
                        ByteSourceError::Fatal { code, .. } => {
                            events.extend(
                                self.engine
                                    .apply(EngineCommand::Network(NetworkEvent::Fatal { code }))?
                                    .events,
                            );
                        }
                        _ => {
                            events.extend(
                                self.engine
                                    .apply(EngineCommand::Network(NetworkEvent::Fatal { code: 0 }))?
                                    .events,
                            );
                        }
                    }
                    None
                }
                ByteSourceEvent::Live => None,
            }
        };

        if let Some(ref data) = data_opt {
            let time =
                MediaTime::from_pts_dts(Timestamp::new(0), Timestamp::new(0), TimeBase::DEFAULT);
            let input = Input {
                data,
                time,
                codec: self.track.codec,
                track_id: self.track.id,
            };
            match self.decoder.decode(&input) {
                Ok(output) => {
                    self.diagnostics.record(DiagnosticEvent::FrameDecoded {
                        track_id: self.track.id,
                        codec: self.track.codec,
                    });
                    self.route_output(&output)?;
                }
                Err(e) => {
                    self.diagnostics.record(DiagnosticEvent::Error {
                        stage: "decoder",
                        code: 0,
                    });
                    return Err(NativePlayerError::Abi(e));
                }
            }
        }

        Ok(events)
    }

    /// Current diagnostics state.
    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    /// Current lifecycle log.
    pub fn lifecycle(&self) -> &LifecycleSoak {
        &self.lifecycle
    }

    fn route_output(&mut self, output: &Output) -> Result<(), NativePlayerError> {
        match self.track.kind {
            TrackKind::Video => {
                if let Some(r) = self.renderer.as_mut() {
                    r.render(output)?;
                    self.diagnostics.record(DiagnosticEvent::FrameRendered {
                        track_id: self.track.id,
                    });
                }
            }
            TrackKind::Audio => {
                if let Some(a) = self.audio_sink.as_mut() {
                    a.play(output)?;
                    let samples = self.audio_samples(output);
                    self.diagnostics.record(DiagnosticEvent::AudioPlayed {
                        track_id: self.track.id,
                        samples,
                    });
                }
            }
            TrackKind::Data => {}
        }
        Ok(())
    }

    fn audio_samples(&self, output: &Output) -> u64 {
        if let Some(target) = self.audio {
            let format = target.format;
            let bytes_per_sample = format.sample_format.bytes_per_sample();
            let channels = format.channel_layout.channels();
            if bytes_per_sample == 0 || channels == 0 {
                return 0;
            }
            let frame_size = (bytes_per_sample * channels) as u64;
            output.data.len() as u64 / frame_size
        } else {
            0
        }
    }
}

fn byte_source_error_code(e: &ByteSourceError) -> u32 {
    match e {
        ByteSourceError::Fatal { code, .. } => *code,
        ByteSourceError::Retryable { .. } => 100,
        ByteSourceError::NotStarted => 101,
        ByteSourceError::Eof | ByteSourceError::WouldBlock | ByteSourceError::Cancelled => 0,
    }
}

/// Builds a `NativePlayer` from a configuration and optional source override.
pub struct NativePlayerBuilder {
    config: NativePlayerConfig,
    source: Option<Box<dyn ByteSource>>,
    memory_data: Option<Vec<u8>>,
    chunk_size: usize,
}

impl NativePlayerBuilder {
    /// Create a builder for the given configuration.
    pub fn new(config: NativePlayerConfig) -> Self {
        Self {
            config,
            source: None,
            memory_data: None,
            chunk_size: 8192,
        }
    }

    /// Override the byte source (useful for tests and custom transports).
    pub fn with_source(mut self, source: impl ByteSource + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// Provide an in-memory byte buffer for `memory://` sources.
    pub fn with_memory_source(mut self, data: Vec<u8>) -> Self {
        self.memory_data = Some(data);
        self
    }

    /// Set the chunk size for `memory://` sources.
    pub fn chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = size.max(1);
        self
    }

    /// Negotiate a backend plan, create the platform components and return a
    /// ready-to-load `NativePlayer`.
    pub fn build(self) -> Result<NativePlayer, NativePlayerError> {
        let mut decoder_registry = CapabilityRegistry::new();
        decoder_registry.register(DefaultProbe);
        let renderer_registry = RendererRegistry::with_probe(DefaultRendererProbe);
        let audio_registry = AudioSinkRegistry::with_probe(NullAudioSinkProbe);

        let plan = BackendPlan::negotiate(
            &self.config.url,
            self.config.track.codec,
            self.config.video,
            self.config.audio,
            &decoder_registry,
            &renderer_registry,
            &audio_registry,
        )?;

        let source: Box<dyn ByteSource> = match self.source {
            Some(s) => s,
            None => match plan.transport {
                crate::native::negotiator::TransportKind::Memory => {
                    let data = self.memory_data.unwrap_or_default();
                    Box::new(MemoryByteSource::new(data, self.chunk_size))
                }
                crate::native::negotiator::TransportKind::Tcp
                | crate::native::negotiator::TransportKind::Http
                | crate::native::negotiator::TransportKind::WebSocket => {
                    Box::new(NativeByteSource::new()?)
                }
            },
        };

        let video_info = self.config.video.map(|v| (v.width, v.height, v.fps));
        let decoder = Box::new(NativeDecoder::from_registry(
            &decoder_registry,
            self.config.track.codec,
            video_info,
        )?) as Box<dyn Decoder>;

        let renderer = if let Some(v) = self.config.video {
            Some(Box::new(NativeRenderer::from_registry(
                &renderer_registry,
                v.format,
                v.width,
                v.height,
            )?) as Box<dyn Renderer>)
        } else {
            None
        };

        let audio_sink = if let Some(a) = self.config.audio {
            Some(create_sink(&audio_registry, &a.format)? as Box<dyn AudioSink + Send>)
        } else {
            None
        };

        let mut player = NativePlayer::new(
            source,
            self.config.track,
            self.config.video,
            self.config.audio,
            decoder,
            renderer,
            audio_sink,
        );

        if self.config.autoplay {
            let _ = player.load(&self.config.url)?;
            let _ = player.play()?;
        }

        Ok(player)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_native_audio::NullAudioSink;
    use cheetah_media_native_decoder::{G711Decoder, NativeDecoder, g711::G711Kind};
    use cheetah_media_types::{
        AudioFormat, ChannelLayout, CodecConfig, CodecId, SampleFormat, TrackId,
    };

    fn g711_track() -> TrackInfo {
        TrackInfo {
            id: TrackId::new(1).unwrap(),
            kind: TrackKind::Audio,
            codec: CodecId::G711A,
            timebase: TimeBase::DEFAULT,
            codec_config: CodecConfig::None,
            video_format: None,
            audio_format: Some(AudioFormat {
                sample_format: SampleFormat::S16,
                sample_rate: 8000,
                channel_layout: ChannelLayout::Mono,
                sample_count: 0,
            }),
            generation: 1,
        }
    }

    fn audio_target() -> AudioTarget {
        AudioTarget {
            format: AudioFormat {
                sample_format: SampleFormat::S16,
                sample_rate: 8000,
                channel_layout: ChannelLayout::Mono,
                sample_count: 0,
            },
        }
    }

    #[test]
    fn g711_memory_smoke() {
        let data: Vec<u8> = (0..160).map(|i| i as u8).collect();
        let source = MemoryByteSource::new(data.clone(), 80);
        let track = g711_track();
        let decoder =
            NativeDecoder::with_backends(vec![Box::new(G711Decoder::new(G711Kind::ALaw))]);
        let audio_sink = NullAudioSink::new(8000, 1, SampleFormat::S16);

        let mut player = NativePlayer::new(
            Box::new(source),
            track,
            None,
            Some(audio_target()),
            Box::new(decoder) as Box<dyn Decoder>,
            None,
            Some(Box::new(audio_sink) as Box<dyn AudioSink + Send>),
        );

        let load_events = player.load("memory://").unwrap();
        assert!(load_events.iter().any(|e| matches!(
            e,
            EngineEvent::StateChanged {
                to: PlayerState::Preroll,
                ..
            }
        )));

        let play_events = player.play().unwrap();
        assert!(play_events.iter().any(|e| matches!(
            e,
            EngineEvent::StateChanged {
                to: PlayerState::Playing,
                ..
            }
        )));

        // Two chunks of 80 bytes each.
        player.tick().unwrap();
        player.tick().unwrap();
        // Third read hits EOF.
        let eof_events = player.tick().unwrap();
        assert!(eof_events.iter().any(|e| matches!(e, EngineEvent::Eof)));

        player.stop().unwrap();
        let counters = player.diagnostics().counters();
        assert_eq!(counters.decoded, 2);
        assert!(counters.audio_samples > 0);

        player.destroy().unwrap();
    }

    #[test]
    fn play_before_load_is_rejected() {
        let track = g711_track();
        let decoder =
            NativeDecoder::with_backends(vec![Box::new(G711Decoder::new(G711Kind::ALaw))]);
        let audio_sink = NullAudioSink::new(8000, 1, SampleFormat::S16);
        let mut player = NativePlayer::new(
            Box::new(MemoryByteSource::new(Vec::new(), 80)),
            track,
            None,
            Some(audio_target()),
            Box::new(decoder) as Box<dyn Decoder>,
            None,
            Some(Box::new(audio_sink) as Box<dyn AudioSink + Send>),
        );
        let events = player.play().unwrap();
        assert!(events.iter().any(|e| matches!(e, EngineEvent::Error(_))));
    }

    #[test]
    fn invalid_url_fails_to_build() {
        let config = NativePlayerConfig {
            url: "ftp://example.com".into(),
            track: g711_track(),
            video: None,
            audio: Some(audio_target()),
            autoplay: false,
        };
        assert!(NativePlayerBuilder::new(config).build().is_err());
    }

    #[test]
    fn builder_with_memory_source() {
        let data: Vec<u8> = (0..80).map(|i| i as u8).collect();
        let config = NativePlayerConfig {
            url: "memory://".into(),
            track: g711_track(),
            video: None,
            audio: Some(audio_target()),
            autoplay: false,
        };
        let mut player = NativePlayerBuilder::new(config)
            .with_memory_source(data)
            .chunk_size(40)
            .build()
            .unwrap();
        player.load("memory://").unwrap();
        player.play().unwrap();
        player.tick().unwrap();
        player.tick().unwrap();
        assert!(player.diagnostics().counters().decoded >= 1);
        player.stop().unwrap();
        player.destroy().unwrap();
    }

    #[test]
    fn stop_before_play_is_valid() {
        let track = g711_track();
        let decoder =
            NativeDecoder::with_backends(vec![Box::new(G711Decoder::new(G711Kind::ALaw))]);
        let audio_sink = NullAudioSink::new(8000, 1, SampleFormat::S16);
        let mut player = NativePlayer::new(
            Box::new(MemoryByteSource::new(Vec::new(), 80)),
            track,
            None,
            Some(audio_target()),
            Box::new(decoder) as Box<dyn Decoder>,
            None,
            Some(Box::new(audio_sink) as Box<dyn AudioSink + Send>),
        );
        player.load("memory://").unwrap();
        player.stop().unwrap();
        player.destroy().unwrap();
    }

    #[test]
    fn repeated_control_commands_are_idempotent() {
        let data: Vec<u8> = (0..80).map(|i| i as u8).collect();
        let config = NativePlayerConfig {
            url: "memory://".into(),
            track: g711_track(),
            video: None,
            audio: Some(audio_target()),
            autoplay: false,
        };
        let mut player = NativePlayerBuilder::new(config)
            .with_memory_source(data)
            .chunk_size(40)
            .build()
            .unwrap();
        player.load("memory://").unwrap();
        player.play().unwrap();
        player.play().unwrap();
        player.pause().unwrap();
        player.pause().unwrap();
        player.play().unwrap();
        player.stop().unwrap();
        player.stop().unwrap();
        player.destroy().unwrap();
    }
}

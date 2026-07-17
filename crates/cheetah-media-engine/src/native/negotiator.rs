//! Native backend capability negotiation.

use alloc::string::String;

use cheetah_media_native_audio::{AudioSinkRegistry, PlatformAudioSink};
use cheetah_media_native_decoder::{CapabilityRegistry, PlatformApi};
use cheetah_media_native_renderer::{PixelFormat, PlatformRenderer, RendererRegistry};
use cheetah_media_types::{AudioFormat, CodecId};

/// Transport family selected for a URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    /// In-memory source for tests.
    Memory,
    /// TCP socket.
    Tcp,
    /// HTTP/HTTPS progressive download.
    Http,
    /// WebSocket.
    WebSocket,
}

/// Video output target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoTarget {
    pub format: PixelFormat,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

/// Audio output target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioTarget {
    pub format: AudioFormat,
}

/// Decoded backend selected for each pipeline stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendPlan {
    pub transport: TransportKind,
    pub decoder: PlatformApi,
    pub renderer: Option<PlatformRenderer>,
    pub audio: Option<PlatformAudioSink>,
}

impl BackendPlan {
    /// Negotiate a backend plan from registries and a target configuration.
    ///
    /// `url` may be `memory://`, `tcp://`, `http(s)://` or `ws(s)://`.
    /// `video` and `audio` are optional; at least one must be provided.
    pub fn negotiate(
        url: &str,
        codec: CodecId,
        video: Option<VideoTarget>,
        audio: Option<AudioTarget>,
        decoder_registry: &CapabilityRegistry,
        renderer_registry: &RendererRegistry,
        audio_registry: &AudioSinkRegistry,
    ) -> Result<Self, NegotiationError> {
        if video.is_none() && audio.is_none() {
            return Err(NegotiationError::NoTarget);
        }

        let transport = transport_kind(url)?;

        let decoder = if let Some(VideoTarget {
            width, height, fps, ..
        }) = video
        {
            decoder_registry
                .select(codec, width, height, fps)
                .unwrap_or(PlatformApi::Software)
        } else {
            decoder_registry
                .select_audio(codec)
                .unwrap_or(PlatformApi::Software)
        };

        let renderer = video.map(|v| {
            renderer_registry
                .select(v.format, v.width, v.height)
                .unwrap_or(PlatformRenderer::Software)
        });

        let audio_sink = audio.map(|a| {
            audio_registry
                .select(&a.format)
                .unwrap_or(PlatformAudioSink::Null)
        });

        Ok(Self {
            transport,
            decoder,
            renderer,
            audio: audio_sink,
        })
    }
}

/// Errors returned during backend negotiation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NegotiationError {
    /// No video or audio target was supplied.
    NoTarget,
    /// The URL scheme is not supported.
    UnsupportedUrl { url: String },
}

impl core::fmt::Display for NegotiationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoTarget => write!(f, "no video or audio target provided"),
            Self::UnsupportedUrl { url } => write!(f, "unsupported URL scheme: {}", url),
        }
    }
}

fn transport_kind(url: &str) -> Result<TransportKind, NegotiationError> {
    if let Some(scheme) = url.split("://").next() {
        match scheme {
            "memory" => Ok(TransportKind::Memory),
            "tcp" => Ok(TransportKind::Tcp),
            "http" | "https" => Ok(TransportKind::Http),
            "ws" | "wss" => Ok(TransportKind::WebSocket),
            _ => Err(NegotiationError::UnsupportedUrl { url: url.into() }),
        }
    } else {
        Err(NegotiationError::UnsupportedUrl { url: url.into() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_native_audio::NullAudioSinkProbe;
    use cheetah_media_native_decoder::{CapabilityRegistry, SoftwareProbe};
    use cheetah_media_native_renderer::SoftwareRendererProbe;
    use cheetah_media_types::{ChannelLayout, SampleFormat};

    fn decoder_registry() -> CapabilityRegistry {
        let mut reg = CapabilityRegistry::new();
        reg.register(SoftwareProbe);
        reg
    }

    fn audio_target() -> AudioTarget {
        AudioTarget {
            format: AudioFormat {
                sample_format: SampleFormat::S16,
                sample_rate: 48000,
                channel_layout: ChannelLayout::Stereo,
                sample_count: 0,
            },
        }
    }

    #[test]
    fn negotiate_audio_path() {
        let dec = decoder_registry();
        let ren = RendererRegistry::with_probe(SoftwareRendererProbe);
        let aud = AudioSinkRegistry::with_probe(NullAudioSinkProbe);
        let plan = BackendPlan::negotiate(
            "memory://",
            CodecId::G711A,
            None,
            Some(audio_target()),
            &dec,
            &ren,
            &aud,
        )
        .unwrap();
        assert_eq!(plan.transport, TransportKind::Memory);
        assert_eq!(plan.decoder, PlatformApi::Software);
        assert_eq!(plan.audio, Some(PlatformAudioSink::Null));
        assert!(plan.renderer.is_none());
    }

    #[test]
    fn unsupported_url_is_rejected() {
        let dec = decoder_registry();
        let ren = RendererRegistry::with_probe(SoftwareRendererProbe);
        let aud = AudioSinkRegistry::with_probe(NullAudioSinkProbe);
        assert!(
            BackendPlan::negotiate(
                "ftp://example.com",
                CodecId::G711A,
                None,
                Some(audio_target()),
                &dec,
                &ren,
                &aud,
            )
            .is_err()
        );
    }

    #[test]
    fn no_target_is_rejected() {
        let dec = decoder_registry();
        let ren = RendererRegistry::with_probe(SoftwareRendererProbe);
        let aud = AudioSinkRegistry::with_probe(NullAudioSinkProbe);
        assert!(
            BackendPlan::negotiate("memory://", CodecId::G711A, None, None, &dec, &ren, &aud,)
                .is_err()
        );
    }
}

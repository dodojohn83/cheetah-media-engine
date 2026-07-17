//! Registries for selecting broadcast pipeline backends.
//!
//! Registries map capture kinds, codecs and URL schemes to concrete
//! implementations. They are intentionally minimal in WP-70; real probing and
//! prioritization will be added when concrete backends arrive.

use alloc::boxed::Box;
use alloc::vec::Vec;

use cheetah_media_types::CodecId;

use crate::broadcast::encoder::Encoder;
use crate::broadcast::publisher::PublisherBackend;
use crate::broadcast::source::CaptureSource;

/// Registry of capture sources.
pub struct CaptureSourceRegistry {
    sources: Vec<Box<dyn CaptureSource>>,
}

impl CaptureSourceRegistry {
    /// Create an empty registry.
    pub const fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Register a capture source.
    pub fn register(&mut self, source: Box<dyn CaptureSource>) {
        self.sources.push(source);
    }

    /// Find a source by `kind`.
    pub fn select(&self, kind: &str) -> Option<&dyn CaptureSource> {
        self.sources
            .iter()
            .find(|s| s.kind() == kind)
            .map(|s| s.as_ref())
    }

    /// Number of registered sources.
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// True if no sources are registered.
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

impl Default for CaptureSourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry of encoders.
pub struct EncoderRegistry {
    encoders: Vec<Box<dyn Encoder>>,
}

impl EncoderRegistry {
    /// Create an empty registry.
    pub const fn new() -> Self {
        Self {
            encoders: Vec::new(),
        }
    }

    /// Register an encoder.
    pub fn register(&mut self, encoder: Box<dyn Encoder>) {
        self.encoders.push(encoder);
    }

    /// Select the highest-priority encoder that supports `codec`.
    pub fn select(&self, codec: CodecId) -> Option<&dyn Encoder> {
        self.encoders
            .iter()
            .filter(|e| e.as_ref().supports(codec))
            .max_by_key(|e| encoder_priority(e.as_ref()))
            .map(|e| e.as_ref())
    }

    /// Number of registered encoders.
    pub fn len(&self) -> usize {
        self.encoders.len()
    }

    /// True if no encoders are registered.
    pub fn is_empty(&self) -> bool {
        self.encoders.is_empty()
    }
}

impl Default for EncoderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn encoder_priority(_encoder: &dyn Encoder) -> i32 {
    // For WP-70 every registered encoder is treated equally; priority will be
    // driven by `EncoderCapability` once probes are added.
    0
}

/// Registry of publisher backends.
pub struct PublisherBackendRegistry {
    backends: Vec<Box<dyn PublisherBackend>>,
}

impl PublisherBackendRegistry {
    /// Create an empty registry.
    pub const fn new() -> Self {
        Self {
            backends: Vec::new(),
        }
    }

    /// Register a publisher backend.
    pub fn register(&mut self, backend: Box<dyn PublisherBackend>) {
        self.backends.push(backend);
    }

    /// Select a backend whose `kind()` matches the URL scheme prefix of `url`.
    pub fn select(&self, url: &str) -> Option<&dyn PublisherBackend> {
        let scheme = url.split("://").next().unwrap_or(url);
        self.backends
            .iter()
            .find(|b| b.kind() == scheme)
            .map(|b| b.as_ref())
    }

    /// Number of registered backends.
    pub fn len(&self) -> usize {
        self.backends.len()
    }

    /// True if no backends are registered.
    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }
}

impl Default for PublisherBackendRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broadcast::encoder::UnsupportedEncoder;
    use crate::broadcast::publisher::UnsupportedPublisherBackend;
    use crate::broadcast::source::UnsupportedCaptureSource;

    #[test]
    fn capture_registry_selects_by_kind() {
        let mut reg = CaptureSourceRegistry::new();
        reg.register(Box::new(UnsupportedCaptureSource));
        assert!(reg.select("unsupported").is_some());
        assert!(reg.select("camera").is_none());
    }

    #[test]
    fn encoder_registry_selects_by_codec() {
        let mut reg = EncoderRegistry::new();
        reg.register(Box::new(UnsupportedEncoder));
        // UnsupportedEncoder rejects all codecs.
        assert!(reg.select(CodecId::H264).is_none());
    }

    #[test]
    fn publisher_registry_selects_by_scheme() {
        let mut reg = PublisherBackendRegistry::new();
        reg.register(Box::new(UnsupportedPublisherBackend));
        assert!(reg.select("unsupported://x").is_some());
        assert!(reg.select("webrtc://x").is_none());
    }
}

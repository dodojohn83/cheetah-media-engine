//! Android `MediaCodec` capability probe.
//!
//! When the Android NDK is not linked (the default for host/x86 CI builds) the
//! probe reports no capabilities. It never claims support that cannot be
//! verified at build or runtime.

use alloc::vec::Vec;

use cheetah_media_native_decoder::capability::{BackendKind, DecoderCapability, PlatformApi};
use cheetah_media_native_decoder::probe::Probe;

/// Probe for Android `MediaCodec` decoder / encoder capabilities.
pub struct AndroidMediaCodecProbe;

impl Probe for AndroidMediaCodecProbe {
    fn name(&self) -> &'static str {
        "android-mediacodec"
    }

    fn probe(&self) -> Vec<DecoderCapability> {
        // No verified capabilities until the Android NDK is linked and runtime
        // probing is implemented (WP-64). Returning empty avoids fake support.
        Vec::new()
    }
}

impl AndroidMediaCodecProbe {
    /// Return the platform API token associated with this probe.
    pub const fn api() -> PlatformApi {
        PlatformApi::AndroidMediaCodec
    }

    /// Return the backend kind (hardware-backed on Android).
    pub const fn kind() -> BackendKind {
        BackendKind::Hardware
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_reports_no_capabilities_when_ndk_not_linked() {
        let probe = AndroidMediaCodecProbe;
        let caps = probe.probe();
        assert!(caps.is_empty(), "probe must not claim unverified support");
        assert_eq!(probe.name(), "android-mediacodec");
    }

    #[test]
    fn api_and_kind_are_stable() {
        assert_eq!(
            AndroidMediaCodecProbe::api(),
            PlatformApi::AndroidMediaCodec
        );
        assert_eq!(AndroidMediaCodecProbe::kind(), BackendKind::Hardware);
    }
}

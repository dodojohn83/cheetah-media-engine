//! Test fixtures and contract harness for the Cheetah media engine.

use alloc::vec::Vec;
use cheetah_media_types::{CodecId, MediaTime, TimeBase, TrackKind};
use serde::Deserialize;

extern crate alloc;

pub mod compare;
pub mod store;
pub mod validate;
pub use compare::{
    CompareOptions, Diff, compare_audio_frames, compare_packet_lists, compare_packets,
    compare_video_frames,
};
pub use store::{FixtureStatus, FixtureStore};
pub use validate::{FixtureError, validate_manifest};

/// A minimal test fixture description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixture {
    pub id: &'static str,
    pub kind: TrackKind,
    pub codec: CodecId,
    pub duration_ms: u64,
}

impl Fixture {
    /// Create a standard H.264 video fixture.
    pub fn h264_video(id: &'static str, duration_ms: u64) -> Self {
        Self {
            id,
            kind: TrackKind::Video,
            codec: CodecId::H264,
            duration_ms,
        }
    }

    /// Create a standard AAC audio fixture.
    pub fn aac_audio(id: &'static str, duration_ms: u64) -> Self {
        Self {
            id,
            kind: TrackKind::Audio,
            codec: CodecId::Aac,
            duration_ms,
        }
    }

    /// Create a G.711 A-law mono fixture.
    pub fn g711a_audio(id: &'static str, duration_ms: u64) -> Self {
        Self {
            id,
            kind: TrackKind::Audio,
            codec: CodecId::G711A,
            duration_ms,
        }
    }

    /// Create a G.711 mu-law mono fixture.
    pub fn g711u_audio(id: &'static str, duration_ms: u64) -> Self {
        Self {
            id,
            kind: TrackKind::Audio,
            codec: CodecId::G711U,
            duration_ms,
        }
    }
}

/// Generate a deterministic timestamp sequence for property tests.
pub fn timestamp_sequence(start: i64, count: usize, step: i64) -> impl Iterator<Item = MediaTime> {
    (0..count).map(move |i| {
        let t = start + i as i64 * step;
        MediaTime::from_ticks(Some(t), Some(t), None, TimeBase::DEFAULT)
    })
}

/// Source metadata for a fixture.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Synthetic,
    Download,
    Recorded,
}

/// Fixture source.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct FixtureSource {
    pub r#type: SourceType,
    #[serde(default)]
    pub generator: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub commit: Option<String>,
}

/// Fixture manifest entry.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct FixtureManifestEntry {
    pub id: String,
    pub description: String,
    pub source: FixtureSource,
    pub license: String,
    pub hash: String,
    pub protocol: String,
    pub codec: String,
    #[serde(default)]
    pub resolution: Option<String>,
    #[serde(default)]
    pub frame_rate: Option<u32>,
    #[serde(default)]
    pub sample_rate: Option<u32>,
    #[serde(default)]
    pub channels: Option<u32>,
    pub duration_ms: u64,
    #[serde(default)]
    pub anomaly: Option<String>,
    #[serde(default)]
    pub expected: Option<String>,
}

/// Full fixture manifest.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct FixtureManifest {
    pub schema_version: String,
    pub fixtures: Vec<FixtureManifestEntry>,
}

/// Load the fixture manifest from the provided JSON string.
pub fn load_manifest(json: &str) -> Result<FixtureManifest, serde_json::Error> {
    serde_json::from_str(json)
}

/// Load the workspace fixture manifest from the embedded file.
pub fn workspace_manifest() -> Result<FixtureManifest, serde_json::Error> {
    const MANIFEST: &str = include_str!("../../../testing/fixtures/manifest.json");
    load_manifest(MANIFEST)
}

/// Find a fixture by `id`.
pub fn find_fixture_by_id<'a>(
    manifest: &'a FixtureManifest,
    id: &str,
) -> Option<&'a FixtureManifestEntry> {
    manifest.fixtures.iter().find(|f| f.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_describes_video() {
        let f = Fixture::h264_video("f1", 1000);
        assert_eq!(f.codec, CodecId::H264);
    }

    #[test]
    fn timestamp_sequence_is_deterministic() {
        let times: Vec<_> = timestamp_sequence(0, 3, 33).collect();
        assert_eq!(times[0].pts.map(|p| p.ticks()), Some(0));
        assert_eq!(times[1].pts.map(|p| p.ticks()), Some(33));
    }

    #[test]
    fn workspace_manifest_loads() {
        let manifest = workspace_manifest().expect("manifest parses");
        assert_eq!(manifest.schema_version, "1.0");
        assert!(!manifest.fixtures.is_empty());
        let h264 = find_fixture_by_id(&manifest, "h264-1280x720-30fps-2s").expect("fixture exists");
        assert_eq!(h264.codec, "h264");
        assert_eq!(h264.license, "MIT-0");
    }

    #[test]
    fn g711a_fixture_has_g711a_codec() {
        let f = Fixture::g711a_audio("g711a", 500);
        assert_eq!(f.codec, CodecId::G711A);
    }
}

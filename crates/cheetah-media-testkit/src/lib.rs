//! Test fixtures and contract harness for the Cheetah media engine.

use cheetah_media_types::{CodecId, MediaTime, TrackKind};

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
}

/// Generate a deterministic timestamp sequence for property tests.
pub fn timestamp_sequence(start: i64, count: usize, step: i64) -> impl Iterator<Item = MediaTime> {
    (0..count).map(move |i| {
        let t = start + i as i64 * step;
        MediaTime::new(t, t, 1000)
    })
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
        assert_eq!(times[0].pts, 0);
        assert_eq!(times[1].pts, 33);
    }
}

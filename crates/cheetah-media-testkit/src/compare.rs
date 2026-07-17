//! Field-level comparison helpers for golden/contract tests.
//!
//! Comparisons ignore `Cow` ownership and buffer addresses; they compare
//! payload bytes, metadata fields and timestamps within a configurable
//! tolerance. The first detected difference is returned so that failures are
//! easy to diagnose without relying on `Debug` formatting or hash order.

use alloc::string::String;
use cheetah_media_types::{AudioFrame, MediaPacket, MediaTime, Timestamp, VideoFrame};

/// A single difference between two values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    pub path: String,
    pub expected: String,
    pub actual: String,
}

impl Diff {
    fn new(path: &str, expected: impl core::fmt::Debug, actual: impl core::fmt::Debug) -> Self {
        Self {
            path: String::from(path),
            expected: format!("{:?}", expected),
            actual: format!("{:?}", actual),
        }
    }
}

/// Options for comparing timestamps and payload bytes.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompareOptions {
    /// Accepted difference between timestamps in the same timebase (in ticks).
    pub time_tolerance_ticks: i64,
    /// When true, the duration field is ignored.
    pub ignore_duration: bool,
}

/// Compare two media packets and return the first difference, if any.
pub fn compare_packets<'a>(
    path: &str,
    expected: &MediaPacket<'a>,
    actual: &MediaPacket<'a>,
    opts: CompareOptions,
) -> Option<Diff> {
    if expected.track_id != actual.track_id {
        return Some(Diff::new(
            &format!("{path}.track_id"),
            expected.track_id,
            actual.track_id,
        ));
    }
    if expected.stream_epoch != actual.stream_epoch {
        return Some(Diff::new(
            &format!("{path}.stream_epoch"),
            expected.stream_epoch,
            actual.stream_epoch,
        ));
    }
    if expected.sequence != actual.sequence {
        return Some(Diff::new(
            &format!("{path}.sequence"),
            expected.sequence,
            actual.sequence,
        ));
    }
    if expected.payload.as_ref() != actual.payload.as_ref() {
        return Some(Diff::new(
            &format!("{path}.payload"),
            expected.payload.len(),
            actual.payload.len(),
        ));
    }
    if let Some(d) = compare_media_time(&format!("{path}.time"), &expected.time, &actual.time, opts)
    {
        return Some(d);
    }
    if expected.flags.is_keyframe != actual.flags.is_keyframe {
        return Some(Diff::new(
            &format!("{path}.flags.is_keyframe"),
            expected.flags.is_keyframe,
            actual.flags.is_keyframe,
        ));
    }
    if expected.flags.is_corrupt != actual.flags.is_corrupt {
        return Some(Diff::new(
            &format!("{path}.flags.is_corrupt"),
            expected.flags.is_corrupt,
            actual.flags.is_corrupt,
        ));
    }
    if expected.flags.is_discontinuity != actual.flags.is_discontinuity {
        return Some(Diff::new(
            &format!("{path}.flags.is_discontinuity"),
            expected.flags.is_discontinuity,
            actual.flags.is_discontinuity,
        ));
    }
    None
}

fn compare_media_time(
    path: &str,
    expected: &MediaTime,
    actual: &MediaTime,
    opts: CompareOptions,
) -> Option<Diff> {
    if expected.timebase != actual.timebase {
        return Some(Diff::new(
            &format!("{path}.timebase"),
            expected.timebase,
            actual.timebase,
        ));
    }
    let tol = opts.time_tolerance_ticks;
    if !same_timestamp_opt(expected.pts, actual.pts, tol) {
        return Some(Diff::new(
            &format!("{path}.pts"),
            expected.pts.map(|t| t.ticks()),
            actual.pts.map(|t| t.ticks()),
        ));
    }
    if !same_timestamp_opt(expected.dts, actual.dts, tol) {
        return Some(Diff::new(
            &format!("{path}.dts"),
            expected.dts.map(|t| t.ticks()),
            actual.dts.map(|t| t.ticks()),
        ));
    }
    if !opts.ignore_duration && !same_timestamp_opt(expected.duration, actual.duration, tol) {
        return Some(Diff::new(
            &format!("{path}.duration"),
            expected.duration.map(|t| t.ticks()),
            actual.duration.map(|t| t.ticks()),
        ));
    }
    None
}

fn same_timestamp_opt(a: Option<Timestamp>, b: Option<Timestamp>, tol: i64) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a.ticks() - b.ticks()).abs() <= tol,
        (None, None) => true,
        _ => false,
    }
}

/// Compare two `VideoFrame` values and return the first difference, if any.
pub fn compare_video_frames(
    path: &str,
    expected: &VideoFrame,
    actual: &VideoFrame,
    opts: CompareOptions,
) -> Option<Diff> {
    if expected.format != actual.format {
        return Some(Diff::new(
            &format!("{path}.format"),
            expected.format,
            actual.format,
        ));
    }
    if expected.payload.as_ref() != actual.payload.as_ref() {
        return Some(Diff::new(
            &format!("{path}.payload"),
            expected.payload.len(),
            actual.payload.len(),
        ));
    }
    if expected.planes.len() != actual.planes.len() {
        return Some(Diff::new(
            &format!("{path}.planes.len"),
            expected.planes.len(),
            actual.planes.len(),
        ));
    }
    for (i, (e, a)) in expected.planes.iter().zip(actual.planes.iter()).enumerate() {
        if e.as_ref() != a.as_ref() {
            return Some(Diff::new(&format!("{path}.planes[{i}]"), e.len(), a.len()));
        }
    }
    if let Some(d) = compare_media_time(
        &format!("{path}.timestamp"),
        &expected.timestamp,
        &actual.timestamp,
        opts,
    ) {
        return Some(d);
    }
    None
}

/// Compare two `AudioFrame` values and return the first difference, if any.
pub fn compare_audio_frames(
    path: &str,
    expected: &AudioFrame,
    actual: &AudioFrame,
    opts: CompareOptions,
) -> Option<Diff> {
    if expected.format != actual.format {
        return Some(Diff::new(
            &format!("{path}.format"),
            expected.format,
            actual.format,
        ));
    }
    if expected.payload.as_ref() != actual.payload.as_ref() {
        return Some(Diff::new(
            &format!("{path}.payload"),
            expected.payload.len(),
            actual.payload.len(),
        ));
    }
    if expected.planes.len() != actual.planes.len() {
        return Some(Diff::new(
            &format!("{path}.planes.len"),
            expected.planes.len(),
            actual.planes.len(),
        ));
    }
    for (i, (e, a)) in expected.planes.iter().zip(actual.planes.iter()).enumerate() {
        if e.as_ref() != a.as_ref() {
            return Some(Diff::new(&format!("{path}.planes[{i}]"), e.len(), a.len()));
        }
    }
    if let Some(d) = compare_media_time(
        &format!("{path}.timestamp"),
        &expected.timestamp,
        &actual.timestamp,
        opts,
    ) {
        return Some(d);
    }
    None
}

/// Compare two slices of packets element-wise and return the first difference.
pub fn compare_packet_lists<'a>(
    expected: &[MediaPacket<'a>],
    actual: &[MediaPacket<'a>],
    opts: CompareOptions,
) -> Option<Diff> {
    if expected.len() != actual.len() {
        return Some(Diff::new(
            "packet_lists.len()",
            expected.len(),
            actual.len(),
        ));
    }
    for (i, (e, a)) in expected.iter().zip(actual.iter()).enumerate() {
        if let Some(d) = compare_packets(&format!("packet[{i}]"), e, a, opts) {
            return Some(d);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_types::{
        BufferRef, MediaTime, SequenceNumber, StreamEpoch, TimeBase, TrackId,
    };

    fn dummy_packet(pts: i64, payload: Vec<u8>) -> MediaPacket<'static> {
        let mut packet = MediaPacket::new(
            BufferRef::from_owned(payload),
            TrackId::new(1).unwrap(),
            StreamEpoch::new(0),
            SequenceNumber::new(0),
            MediaTime::from_ticks(Some(pts), Some(pts), None, TimeBase::DEFAULT),
        );
        packet.flags.is_keyframe = true;
        packet
    }

    #[test]
    fn identical_packets_match() {
        let a = dummy_packet(1000, vec![1, 2, 3]);
        let b = dummy_packet(1000, vec![1, 2, 3]);
        assert!(compare_packets("p", &a, &b, CompareOptions::default()).is_none());
    }

    #[test]
    fn payload_mismatch_reports() {
        let a = dummy_packet(1000, vec![1, 2, 3]);
        let b = dummy_packet(1000, vec![1, 2, 4]);
        let diff = compare_packets("p", &a, &b, CompareOptions::default()).unwrap();
        assert!(diff.path.contains("payload"));
    }

    #[test]
    fn tolerance_allows_small_timestamp_drift() {
        let a = dummy_packet(1000, vec![1, 2, 3]);
        let b = dummy_packet(1002, vec![1, 2, 3]);
        assert!(
            compare_packets(
                "p",
                &a,
                &b,
                CompareOptions {
                    time_tolerance_ticks: 2,
                    ignore_duration: false
                }
            )
            .is_none()
        );
        assert!(
            compare_packets(
                "p",
                &a,
                &b,
                CompareOptions {
                    time_tolerance_ticks: 1,
                    ignore_duration: false
                }
            )
            .is_some()
        );
    }
}

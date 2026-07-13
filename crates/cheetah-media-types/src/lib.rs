//! Shared media types used across the Cheetah media engine.
//!
//! This crate is `no_std` compatible when the `std` feature is disabled. It only
//! depends on `core` and optionally `alloc`.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

/// Identifies a compressed media codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodecId {
    H264,
    H265,
    Aac,
    G711A,
    G711U,
    Mp3,
}

/// Whether a track carries video or audio samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackKind {
    Video,
    Audio,
}

/// A media timestamp pair in a given timescale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaTime {
    /// Presentation timestamp.
    pub pts: i64,
    /// Decode timestamp.
    pub dts: i64,
    /// Timescale (ticks per second).
    pub timescale: u32,
}

impl Default for MediaTime {
    /// Default timestamp with a 1 kHz timescale to avoid division-by-zero in `pts_ms`.
    fn default() -> Self {
        Self::new(0, 0, 1000)
    }
}

impl MediaTime {
    /// Create a new `MediaTime`.
    pub const fn new(pts: i64, dts: i64, timescale: u32) -> Self {
        Self {
            pts,
            dts,
            timescale,
        }
    }

    /// Convert the PTS to milliseconds.
    ///
    /// Returns `0` if the timescale is `0`, preventing division-by-zero panics.
    pub fn pts_ms(&self) -> i64 {
        if self.timescale == 0 {
            return 0;
        }
        self.pts * 1000 / i64::from(self.timescale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_time_pts_ms() {
        let t = MediaTime::new(3000, 3000, 1000);
        assert_eq!(t.pts_ms(), 3000);
    }

    #[test]
    fn media_time_default_has_sane_timescale() {
        let t = MediaTime::default();
        assert_eq!(t.timescale, 1000);
        assert_eq!(t.pts_ms(), 0);
    }

    #[test]
    fn media_time_pts_ms_handles_zero_timescale() {
        let t = MediaTime::new(3000, 3000, 0);
        assert_eq!(t.pts_ms(), 0);
    }
}

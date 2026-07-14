//! GOP cache and random access.
//!
//! A `GopCache` holds groups of pictures starting at an independent decode
//! point (IDR or equivalent). Each GOP stores its compressed packets and is
//! bounded by total bytes, frame count, and duration. Config generation changes
//! invalidate older GOPs, so a new decoder must wait for a fresh random access
//! point before reading from the cache.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use cheetah_media_types::{MediaPacket, StreamEpoch};

/// A group of pictures starting at an independent decode point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gop {
    /// Epoch the GOP belongs to.
    pub epoch: StreamEpoch,
    /// Decoder config generation required to decode this GOP.
    pub config_generation: u64,
    /// Presentation start time in milliseconds, if known.
    pub start_ms: Option<i64>,
    /// Presentation end time in milliseconds, if known.
    pub end_ms: Option<i64>,
    /// Packets in decode order.
    pub packets: Vec<MediaPacket<'static>>,
    /// Total payload bytes in this GOP.
    pub bytes: u64,
    /// Number of frames/packets in this GOP.
    pub frames: usize,
    /// Approximate duration in milliseconds.
    pub duration_ms: u64,
}

impl Gop {
    pub fn new(epoch: StreamEpoch, config_generation: u64, start_ms: Option<i64>) -> Self {
        Self {
            epoch,
            config_generation,
            start_ms,
            end_ms: start_ms,
            packets: Vec::new(),
            bytes: 0,
            frames: 0,
            duration_ms: 0,
        }
    }

    /// Append a packet to the GOP.
    fn push(&mut self, packet: MediaPacket<'static>) {
        let timestamp_ms = packet.time.pts_ms().or_else(|| packet.time.dts_ms());
        if let Some(ms) = timestamp_ms {
            self.end_ms = Some(ms);
            if self.start_ms.is_none() {
                self.start_ms = Some(ms);
            }
            if let Some(start) = self.start_ms {
                let dur = ms.saturating_sub(start).max(0) as u64;
                self.duration_ms = self.duration_ms.max(dur);
            }
        }
        self.bytes += packet.payload.len() as u64;
        self.frames += 1;
        self.packets.push(packet);
    }
}

/// Bounds for the GOP cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GopLimits {
    /// Maximum total bytes across all GOPs.
    pub max_bytes: u64,
    /// Maximum total frames across all GOPs.
    pub max_frames: usize,
    /// Maximum duration in milliseconds across all GOPs.
    pub max_duration_ms: u64,
}

impl Default for GopLimits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_frames: 1024,
            max_duration_ms: 30_000,
        }
    }
}

/// Cache of GOPs for random access and recovery.
#[derive(Debug)]
pub struct GopCache {
    gops: VecDeque<Gop>,
    current: Option<Gop>,
    current_config_generation: u64,
    limits: GopLimits,
}

impl GopCache {
    /// Create a cache with the given bounds.
    pub fn new(limits: GopLimits) -> Self {
        Self {
            gops: VecDeque::new(),
            current: None,
            current_config_generation: 0,
            limits,
        }
    }

    /// Feed a packet into the cache.
    ///
    /// `config_generation` identifies the decoder configuration in effect. When
    /// it changes, old GOPs are discarded and input is ignored until the next
    /// independent frame.
    pub fn feed(&mut self, packet: MediaPacket<'static>, config_generation: u64) {
        // Config generation changed: old GOPs cannot decode with the new config.
        if config_generation != self.current_config_generation {
            self.gops.clear();
            self.current = None;
            self.current_config_generation = config_generation;
        }

        // Ignore data until we reach a new random access point for this config.
        if self.current.is_none() && !packet.flags.is_keyframe {
            return;
        }

        if packet.flags.is_keyframe {
            self.finalize_current();
            let start_ms = packet.time.pts_ms().or_else(|| packet.time.dts_ms());
            self.current = Some(Gop::new(packet.stream_epoch, config_generation, start_ms));
        }

        if let Some(ref mut gop) = self.current {
            gop.push(packet);
            self.trim();
        }
    }

    /// Return the newest GOP whose start time is at or before `time_ms`.
    pub fn random_access(&self, time_ms: i64) -> Option<&Gop> {
        // Search finalized GOPs from newest to oldest, then the current GOP.
        for gop in self.gops.iter().rev() {
            if gop.config_generation != self.current_config_generation {
                continue;
            }
            if gop.start_ms.is_some_and(|s| s <= time_ms) {
                return Some(gop);
            }
        }
        if self.current.as_ref().is_some_and(|c| {
            c.config_generation == self.current_config_generation
                && c.start_ms.is_some_and(|s| s <= time_ms)
        }) {
            return self.current.as_ref();
        }
        None
    }

    /// Return the newest GOP whose start time is at or before the given time,
    /// taking ownership (used to feed a decoder).
    pub fn take_random_access(&mut self, time_ms: i64) -> Option<Gop> {
        let mut best_idx = None;
        for (idx, gop) in self.gops.iter().enumerate().rev() {
            if gop.config_generation != self.current_config_generation {
                continue;
            }
            if gop.start_ms.is_some_and(|s| s <= time_ms) {
                best_idx = Some(idx);
                break;
            }
        }

        if let Some(idx) = best_idx {
            return self.gops.remove(idx);
        }

        if self.current.as_ref().is_some_and(|c| {
            c.config_generation == self.current_config_generation
                && c.start_ms.is_some_and(|s| s <= time_ms)
        }) {
            return self.current.take();
        }
        None
    }

    /// Number of finalized GOPs currently cached.
    pub fn len(&self) -> usize {
        self.gops.len()
    }

    /// True when no finalized GOPs are cached.
    pub fn is_empty(&self) -> bool {
        self.gops.is_empty()
    }

    pub(crate) fn finalize_current(&mut self) {
        if let Some(gop) = self.current.take().filter(|g| g.frames > 0) {
            self.gops.push_back(gop);
        }
    }

    fn total_bytes(&self) -> u64 {
        self.gops.iter().map(|g| g.bytes).sum::<u64>()
            + self.current.as_ref().map_or(0, |g| g.bytes)
    }

    fn total_frames(&self) -> usize {
        self.gops.iter().map(|g| g.frames).sum::<usize>()
            + self.current.as_ref().map_or(0, |g| g.frames)
    }

    fn total_duration_ms(&self) -> u64 {
        self.gops.iter().map(|g| g.duration_ms).sum::<u64>()
            + self.current.as_ref().map_or(0, |g| g.duration_ms)
    }

    fn trim(&mut self) {
        while !self.gops.is_empty()
            && (self.total_bytes() > self.limits.max_bytes
                || self.total_frames() > self.limits.max_frames
                || self.total_duration_ms() > self.limits.max_duration_ms)
        {
            self.gops.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_types::{MediaTime, SequenceNumber, TimeBase, Timestamp, TrackId};

    fn packet(seq: u64, pts_ms: i64, keyframe: bool, stream_epoch: u64) -> MediaPacket<'static> {
        let time = MediaTime::from_pts_dts(
            Timestamp::new(pts_ms),
            Timestamp::new(pts_ms),
            TimeBase::DEFAULT,
        );
        let mut p = MediaPacket::new(
            Vec::from([0u8; 100]),
            TrackId::new(1).unwrap(),
            StreamEpoch::new(stream_epoch),
            SequenceNumber::new(seq),
            time,
        );
        if keyframe {
            p.flags.is_keyframe = true;
        }
        p
    }

    #[test]
    fn starts_gop_on_keyframe() {
        let mut cache = GopCache::new(GopLimits::default());
        cache.feed(packet(0, 0, true, 1), 1);
        cache.feed(packet(1, 40, false, 1), 1);
        assert_eq!(cache.len(), 0);
        cache.finalize_current();
        assert_eq!(cache.len(), 1);
        assert!(cache.random_access(20).is_some());
    }

    #[test]
    fn config_change_ignores_until_keyframe() {
        let mut cache = GopCache::new(GopLimits::default());
        cache.feed(packet(0, 0, true, 1), 1);
        cache.feed(packet(1, 40, false, 1), 1);
        cache.feed(packet(2, 80, false, 1), 2);
        assert_eq!(cache.len(), 0);
        cache.feed(packet(3, 120, true, 1), 2);
        cache.finalize_current();
        assert_eq!(cache.len(), 1);
        assert!(cache.random_access(120).is_some());
    }

    #[test]
    fn respects_byte_limit() {
        let mut cache = GopCache::new(GopLimits {
            max_bytes: 250,
            max_frames: 1000,
            max_duration_ms: 1_000_000,
        });
        for i in 0..5 {
            cache.feed(packet(i, i as i64 * 100, true, 1), 1);
        }
        cache.finalize_current();
        // Each GOP is 100 bytes; with a 250 byte cap only the last two remain.
        assert!(cache.len() <= 2);
    }
}

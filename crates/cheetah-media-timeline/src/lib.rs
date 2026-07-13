//! Media timeline and synchronization primitives.

#![cfg_attr(not(any(test, feature = "std")), no_std)]
extern crate alloc;

use alloc::vec::Vec;
use cheetah_media_types::MediaTime;

/// Ordered timeline of media timestamps.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Timeline {
    entries: Vec<MediaTime>,
}

impl Timeline {
    /// Create an empty timeline.
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Push a timestamp into the timeline.
    ///
    /// Keeps entries sorted by `pts` in ascending order.
    pub fn push(&mut self, time: MediaTime) {
        match self.entries.binary_search_by_key(&time.pts, |t| t.pts) {
            Ok(idx) => self.entries.insert(idx, time),
            Err(idx) => self.entries.insert(idx, time),
        }
    }

    /// Number of entries in the timeline.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if the timeline has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over the timeline.
    pub fn iter(&self) -> impl Iterator<Item = &MediaTime> {
        self.entries.iter()
    }

    /// Find the first entry with a PTS greater than or equal to `pts_ms`.
    pub fn next_after_ms(&self, pts_ms: i64) -> Option<&MediaTime> {
        self.entries.iter().find(|t| t.pts_ms() >= pts_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_sorted_order() {
        let mut tl = Timeline::new();
        tl.push(MediaTime::new(3000, 3000, 1000));
        tl.push(MediaTime::new(1000, 1000, 1000));
        tl.push(MediaTime::new(2000, 2000, 1000));
        let pts: Vec<_> = tl.iter().map(|t| t.pts).collect();
        assert_eq!(pts, vec![1000, 2000, 3000]);
    }

    #[test]
    fn next_after_ms() {
        let mut tl = Timeline::new();
        tl.push(MediaTime::new(1000, 1000, 1000));
        tl.push(MediaTime::new(3000, 3000, 1000));
        let found = tl.next_after_ms(1500).expect("entry found");
        assert_eq!(found.pts, 3000);
    }
}

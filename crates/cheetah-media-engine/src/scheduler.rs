//! Bounded command and sample scheduler.
//!
//! `Scheduler` owns a set of FIFO queues, one per pipeline stage. Each queue
//! has a capacity, a high watermark and a low watermark. Items are tagged with
//! a `Priority` so that control commands, audio clock samples and keyframes are
//! not starved by bulk input. When a queue exceeds capacity, lower-priority
//! items are dropped first and a structured `Overrun` event is emitted.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

/// Priority of a scheduled item. Lower numeric value = higher priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Priority {
    /// Control commands (load, play, stop, destroy).
    Control = 0,
    /// Audio clock / audio samples.
    AudioClock = 1,
    /// Video keyframes and independent decode points.
    Keyframe = 2,
    /// Ordinary data packets/frames.
    Data = 3,
}

/// A scheduler event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerEvent {
    /// An item was dropped from `queue` to make room.
    Dropped { queue: &'static str, count: u64 },
    /// A queue crossed its high watermark.
    HighWatermark { queue: &'static str, level: usize },
    /// A queue dropped back below its low watermark.
    LowWatermark { queue: &'static str, level: usize },
}

/// Configuration for one bounded queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueConfig {
    /// Maximum number of items.
    pub capacity: usize,
    /// High watermark; crossing it produces backpressure.
    pub high: usize,
    /// Low watermark; dropping below it cancels backpressure.
    pub low: usize,
}

impl QueueConfig {
    /// Create a config with `capacity` and watermarks at 75% / 25%.
    pub const fn with_watermarks(capacity: usize) -> Self {
        Self {
            capacity,
            high: capacity * 3 / 4,
            low: capacity / 4,
        }
    }
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self::with_watermarks(64)
    }
}

/// A single bounded priority queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedQueue<T> {
    name: &'static str,
    config: QueueConfig,
    items: VecDeque<(Priority, T)>,
    dropped: u64,
    over_high: bool,
}

impl<T> BoundedQueue<T> {
    /// Create an empty queue.
    pub fn new(name: &'static str, config: QueueConfig) -> Self {
        Self {
            name,
            config,
            items: VecDeque::new(),
            dropped: 0,
            over_high: false,
        }
    }

    /// Push an item with `priority`. If the queue is full, the lowest-priority
    /// item is evicted. Returns the number of items dropped.
    pub fn push(&mut self, priority: Priority, item: T) -> u64 {
        if self.items.len() >= self.config.capacity {
            // Evict the lowest-priority oldest item only if the newcomer has
            // strictly higher priority. Equal or lower priority newcomers are
            // dropped to preserve FIFO order.
            if let Some(&(old_priority, _)) = self.items.back() {
                if priority < old_priority {
                    self.items.pop_back();
                    self.dropped += 1;
                } else {
                    self.dropped += 1;
                    return 1;
                }
            }
        }
        self.items.push_back((priority, item));
        0
    }

    /// Pop the highest-priority item, breaking ties by FIFO order.
    pub fn pop(&mut self) -> Option<T> {
        let (best_idx, _) = self.items.iter().enumerate().min_by_key(|(_, (p, _))| *p)?;
        self.items.remove(best_idx).map(|(_, item)| item)
    }

    /// Current queue depth.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Total number of dropped items.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Poll for watermark events and update the internal state.
    pub fn poll_watermarks(&mut self) -> Vec<SchedulerEvent> {
        let mut events = Vec::new();
        let level = self.items.len();
        if !self.over_high && level >= self.config.high {
            self.over_high = true;
            events.push(SchedulerEvent::HighWatermark {
                queue: self.name,
                level,
            });
        }
        if self.over_high && level <= self.config.low {
            self.over_high = false;
            events.push(SchedulerEvent::LowWatermark {
                queue: self.name,
                level,
            });
        }
        events
    }
}

/// Named queues managed by the scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueueName {
    Input,
    Packet,
    Decode,
    Frame,
    Render,
    Audio,
    Record,
}

impl QueueName {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Packet => "packet",
            Self::Decode => "decode",
            Self::Frame => "frame",
            Self::Render => "render",
            Self::Audio => "audio",
            Self::Record => "record",
        }
    }
}

/// A scheduler that multiplexes several bounded queues.
#[derive(Debug)]
pub struct Scheduler<T> {
    queues: alloc::vec::Vec<BoundedQueue<T>>,
    names: alloc::vec::Vec<QueueName>,
}

impl<T> Scheduler<T> {
    /// Create a scheduler with default 64-slot queues for all known stages.
    pub fn new() -> Self {
        let names = alloc::vec![
            QueueName::Input,
            QueueName::Packet,
            QueueName::Decode,
            QueueName::Frame,
            QueueName::Render,
            QueueName::Audio,
            QueueName::Record,
        ];
        let queues = names
            .iter()
            .map(|n| BoundedQueue::new(n.as_str(), QueueConfig::default()))
            .collect();
        Self { queues, names }
    }

    /// Create a scheduler with explicit per-queue configs.
    pub fn with_configs(configs: &[(QueueName, QueueConfig)]) -> Self {
        let mut scheduler = Self::new();
        for (name, config) in configs {
            if let Some(idx) = scheduler.index_of(*name) {
                scheduler.queues[idx] = BoundedQueue::new(name.as_str(), *config);
            }
        }
        scheduler
    }

    fn index_of(&self, name: QueueName) -> Option<usize> {
        self.names.iter().position(|n| *n == name)
    }

    /// Push `item` into `queue` with `priority`. Returns the number dropped and
    /// any watermark events.
    pub fn push(
        &mut self,
        queue: QueueName,
        priority: Priority,
        item: T,
    ) -> (u64, Vec<SchedulerEvent>) {
        let idx = self.index_of(queue).expect("valid queue");
        let dropped = self.queues[idx].push(priority, item);
        let events = self.queues[idx].poll_watermarks();
        if dropped > 0 {
            let mut events = events;
            events.push(SchedulerEvent::Dropped {
                queue: queue.as_str(),
                count: dropped,
            });
            (dropped, events)
        } else {
            (0, events)
        }
    }

    /// Pop the highest-priority item from all queues, preferring earlier queues
    /// when priorities tie.
    pub fn pop(&mut self) -> Option<(QueueName, T)> {
        let (i, _) = self
            .queues
            .iter()
            .enumerate()
            .filter_map(|(i, q)| q.items.front().map(|(p, _)| (i, *p)))
            .min_by_key(|(_, p)| *p)?;
        self.queues[i].pop().map(|item| (self.names[i], item))
    }

    /// Poll all queues for watermark events.
    pub fn poll_watermarks(&mut self) -> Vec<SchedulerEvent> {
        let mut events = Vec::new();
        for q in &mut self.queues {
            events.extend(q.poll_watermarks());
        }
        events
    }

    /// Total number of items across all queues.
    pub fn total_len(&self) -> usize {
        self.queues.iter().map(|q| q.len()).sum()
    }

    /// Total number of dropped items across all queues.
    pub fn total_dropped(&self) -> u64 {
        self.queues.iter().map(|q| q.dropped()).sum()
    }
}

impl<T> Default for Scheduler<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_accepts_items_up_to_capacity() {
        let mut q = BoundedQueue::new("test", QueueConfig::with_watermarks(2));
        assert_eq!(q.push(Priority::Data, 1), 0);
        assert_eq!(q.push(Priority::Data, 2), 0);
        assert_eq!(q.push(Priority::Data, 3), 1);
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn higher_priority_evicts_lower_priority() {
        let mut q = BoundedQueue::new("test", QueueConfig::with_watermarks(1));
        q.push(Priority::Data, 1);
        assert_eq!(q.push(Priority::Control, 0), 0);
        assert_eq!(q.pop(), Some(0));
    }

    #[test]
    fn lower_priority_dropped_when_full() {
        let mut q = BoundedQueue::new("test", QueueConfig::with_watermarks(1));
        q.push(Priority::Control, 0);
        assert_eq!(q.push(Priority::Data, 1), 1);
        assert_eq!(q.pop(), Some(0));
    }

    #[test]
    fn scheduler_prefers_audio_over_data() {
        let mut s = Scheduler::new();
        s.push(QueueName::Input, Priority::Data, 1);
        s.push(QueueName::Audio, Priority::AudioClock, 2);
        let (_, item) = s.pop().unwrap();
        assert_eq!(item, 2);
    }

    #[test]
    fn watermark_events_fire() {
        let mut q = BoundedQueue::new("test", QueueConfig::with_watermarks(4));
        q.push(Priority::Data, 1);
        q.push(Priority::Data, 2);
        q.push(Priority::Data, 3);
        let events = q.poll_watermarks();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, SchedulerEvent::HighWatermark { .. }))
        );
        q.pop();
        q.pop();
        q.pop();
        let events = q.poll_watermarks();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, SchedulerEvent::LowWatermark { .. }))
        );
    }
}

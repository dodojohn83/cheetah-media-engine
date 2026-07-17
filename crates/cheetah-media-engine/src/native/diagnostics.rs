//! Native player diagnostics and metrics aggregation.

use alloc::string::String;
use alloc::vec::Vec;
use cheetah_media_backend_api::MetricsSink;
use cheetah_media_types::{CodecId, TrackId};

/// A recorded diagnostic event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticEvent {
    /// A backend was selected for a pipeline stage.
    BackendSelected { stage: &'static str, name: String },
    /// A compressed frame was decoded.
    FrameDecoded { track_id: TrackId, codec: CodecId },
    /// A video frame was rendered.
    FrameRendered { track_id: TrackId },
    /// Audio samples were submitted to the sink.
    AudioPlayed { track_id: TrackId, samples: u64 },
    /// Samples or frames were dropped.
    Dropped { stage: &'static str, count: u64 },
    /// An error was reported by a backend.
    Error { stage: &'static str, code: u32 },
}

/// Diagnostic counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiagnosticsCounters {
    pub decoded: u64,
    pub rendered: u64,
    pub audio_samples: u64,
    pub dropped: u64,
    pub errors: u64,
}

/// Aggregates native player diagnostics events and counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostics {
    events: Vec<DiagnosticEvent>,
    counters: DiagnosticsCounters,
    max_events: usize,
}

impl Diagnostics {
    /// Create diagnostics with a bounded event log.
    pub fn with_capacity(max_events: usize) -> Self {
        Self {
            events: Vec::new(),
            counters: DiagnosticsCounters::default(),
            max_events,
        }
    }

    /// Create diagnostics with the default event log size.
    pub fn new() -> Self {
        Self::with_capacity(256)
    }

    /// Record a diagnostic event and update counters.
    pub fn record(&mut self, event: DiagnosticEvent) {
        match &event {
            DiagnosticEvent::FrameDecoded { .. } => self.counters.decoded += 1,
            DiagnosticEvent::FrameRendered { .. } => self.counters.rendered += 1,
            DiagnosticEvent::AudioPlayed { samples, .. } => self.counters.audio_samples += *samples,
            DiagnosticEvent::Dropped { count, .. } => self.counters.dropped += *count,
            DiagnosticEvent::Error { .. } => self.counters.errors += 1,
            _ => {}
        }
        if self.events.len() >= self.max_events {
            self.events.remove(0);
        }
        self.events.push(event);
    }

    /// Note which backend was chosen for a stage.
    pub fn backend_selected(&mut self, stage: &'static str, name: impl Into<String>) {
        self.record(DiagnosticEvent::BackendSelected {
            stage,
            name: name.into(),
        });
    }

    /// Current counters.
    pub fn counters(&self) -> DiagnosticsCounters {
        self.counters
    }

    /// Recent events, oldest first.
    pub fn events(&self) -> &[DiagnosticEvent] {
        &self.events
    }
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::with_capacity(256)
    }
}

impl MetricsSink for Diagnostics {
    fn frame_rendered(&mut self, _pts_ms: i64, _wall_ms: i64) {
        self.record(DiagnosticEvent::FrameRendered {
            track_id: TrackId::new(1).expect("valid track id"),
        });
    }

    fn decoder_error(&mut self, codec: CodecId, error: String) {
        self.record(DiagnosticEvent::Error {
            stage: "decoder",
            code: 0,
        });
        let _ = (codec, error);
    }

    fn dropped(&mut self, queue: &'static str, count: u64) {
        self.record(DiagnosticEvent::Dropped {
            stage: queue,
            count,
        });
    }

    fn backpressure(&mut self, queue: &'static str, level: u32) {
        let _ = (queue, level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_increment() {
        let mut d = Diagnostics::default();
        d.backend_selected("decoder", "software");
        d.record(DiagnosticEvent::FrameDecoded {
            track_id: TrackId::new(1).unwrap(),
            codec: CodecId::G711A,
        });
        d.record(DiagnosticEvent::AudioPlayed {
            track_id: TrackId::new(1).unwrap(),
            samples: 480,
        });
        let c = d.counters();
        assert_eq!(c.decoded, 1);
        assert_eq!(c.audio_samples, 480);
    }

    #[test]
    fn event_log_is_capped() {
        let mut d = Diagnostics::with_capacity(2);
        d.backend_selected("a", "x");
        d.backend_selected("b", "x");
        d.backend_selected("c", "x");
        assert_eq!(d.events().len(), 2);
    }
}

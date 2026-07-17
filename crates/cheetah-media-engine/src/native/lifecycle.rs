//! Native player lifecycle tracking and validation.

use alloc::string::String;
use alloc::vec::Vec;

use crate::state::PlayerState;

/// A recorded lifecycle transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleEvent {
    Created,
    Loaded {
        url: String,
    },
    /// Track and config discovered; ready for `play`.
    Prerolled,
    Played,
    Paused,
    Stopped,
    Destroyed,
    Error {
        stage: &'static str,
        code: u32,
    },
}

/// Tracks `NativePlayer` lifecycle events and detects invalid sequences or
/// leaks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LifecycleSoak {
    events: Vec<LifecycleEvent>,
}

impl LifecycleSoak {
    /// Create an empty lifecycle log.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Record an event.
    pub fn record(&mut self, event: LifecycleEvent) {
        self.events.push(event);
    }

    /// True if `load` has been recorded and `stop`/`destroy` has not yet
    /// followed.
    pub fn is_active(&self) -> bool {
        let mut loaded = false;
        for event in &self.events {
            match event {
                LifecycleEvent::Loaded { .. } | LifecycleEvent::Prerolled => loaded = true,
                LifecycleEvent::Stopped | LifecycleEvent::Destroyed => loaded = false,
                _ => {}
            }
        }
        loaded
    }

    /// True if `destroy` is the last recorded event.
    pub fn is_destroyed(&self) -> bool {
        matches!(self.events.last(), Some(LifecycleEvent::Destroyed))
    }

    /// Validate the recorded sequence up to the current state.
    ///
    /// Returns the first invalid transition, if any.
    pub fn validate(&self) -> Result<(), LifecycleError> {
        let mut state = PlayerState::Idle;
        for event in &self.events {
            match (state, event) {
                (PlayerState::Idle, LifecycleEvent::Created) => {}
                (PlayerState::Idle, LifecycleEvent::Loaded { .. }) => state = PlayerState::Loading,
                (PlayerState::Loading, LifecycleEvent::Prerolled) => state = PlayerState::Preroll,
                (PlayerState::Preroll, LifecycleEvent::Played) => state = PlayerState::Playing,
                (PlayerState::Preroll, LifecycleEvent::Paused) => state = PlayerState::Paused,
                (PlayerState::Playing, LifecycleEvent::Paused) => state = PlayerState::Paused,
                (PlayerState::Paused, LifecycleEvent::Played) => state = PlayerState::Playing,
                (PlayerState::Loading, LifecycleEvent::Stopped)
                | (PlayerState::Preroll, LifecycleEvent::Stopped)
                | (PlayerState::Playing, LifecycleEvent::Stopped)
                | (PlayerState::Paused, LifecycleEvent::Stopped) => state = PlayerState::Idle,
                (PlayerState::Idle, LifecycleEvent::Destroyed) => state = PlayerState::Destroyed,
                (PlayerState::Loading, LifecycleEvent::Destroyed)
                | (PlayerState::Preroll, LifecycleEvent::Destroyed)
                | (PlayerState::Playing, LifecycleEvent::Destroyed)
                | (PlayerState::Paused, LifecycleEvent::Destroyed) => {
                    state = PlayerState::Destroyed
                }
                (_, LifecycleEvent::Error { .. }) => {}
                (s, e) => {
                    return Err(LifecycleError::InvalidTransition {
                        state: s,
                        event: e.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Return all recorded events.
    pub fn events(&self) -> &[LifecycleEvent] {
        &self.events
    }
}

/// Error returned when the lifecycle sequence is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    InvalidTransition {
        state: PlayerState,
        event: LifecycleEvent,
    },
    AlreadyDestroyed,
}

impl core::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidTransition { state, event } => {
                write!(
                    f,
                    "invalid lifecycle transition: {:?} from {:?}",
                    event, state
                )
            }
            Self::AlreadyDestroyed => write!(f, "player already destroyed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_lifecycle_is_valid() {
        let mut soak = LifecycleSoak::new();
        soak.record(LifecycleEvent::Created);
        soak.record(LifecycleEvent::Loaded {
            url: "memory://".into(),
        });
        soak.record(LifecycleEvent::Prerolled);
        soak.record(LifecycleEvent::Played);
        soak.record(LifecycleEvent::Paused);
        soak.record(LifecycleEvent::Played);
        soak.record(LifecycleEvent::Stopped);
        soak.record(LifecycleEvent::Destroyed);
        assert!(soak.validate().is_ok());
        assert!(soak.is_destroyed());
    }

    #[test]
    fn stop_after_preroll_before_play_is_valid() {
        let mut soak = LifecycleSoak::new();
        soak.record(LifecycleEvent::Created);
        soak.record(LifecycleEvent::Loaded {
            url: "memory://".into(),
        });
        soak.record(LifecycleEvent::Prerolled);
        soak.record(LifecycleEvent::Stopped);
        soak.record(LifecycleEvent::Destroyed);
        assert!(soak.validate().is_ok());
    }

    #[test]
    fn play_before_preroll_is_invalid() {
        let mut soak = LifecycleSoak::new();
        soak.record(LifecycleEvent::Created);
        soak.record(LifecycleEvent::Loaded {
            url: "memory://".into(),
        });
        soak.record(LifecycleEvent::Played);
        assert!(soak.validate().is_err());
    }
}

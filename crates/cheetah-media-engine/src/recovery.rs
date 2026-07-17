//! Recovery policy for engine and backend failures.
//!
//! Errors are classified into one of five recovery actions. Each action has a
//! bounded retry budget with exponential backoff so automatic recovery never
//! creates a retry storm.

use alloc::vec::Vec;

/// Recovery action chosen after an error is classified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecoveryAction {
    /// Retry the same stage again (e.g. decoder reset, shader recompile).
    RetrySame,
    /// Rebuild the whole stage while keeping the source/session.
    RebuildStage,
    /// Switch to the next fallback backend for this track.
    FallbackBackend,
    /// Reconnect the source and resume from a safe keyframe.
    ReconnectSource,
    /// Cannot recover; transition to `Failed`.
    Fatal,
}

/// Decision produced by the recovery planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryDecision {
    /// Retry after `delay_ms` milliseconds.
    Retry {
        action: RecoveryAction,
        delay_ms: u64,
        attempts_left: u32,
    },
    /// Escalate to a different action because the current budget is exhausted.
    Escalate { action: RecoveryAction },
    /// Stop trying and declare a fatal failure.
    Fatal,
}

/// Bounded retry budget with exponential backoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryBudget {
    /// Maximum attempts within `window_ms`.
    pub max_attempts: u32,
    /// Time window in milliseconds.
    pub window_ms: u64,
    /// Base backoff in milliseconds.
    pub base_ms: u64,
    /// Maximum backoff in milliseconds.
    pub cap_ms: u64,
}

impl Default for RetryBudget {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            window_ms: 30_000,
            base_ms: 100,
            cap_ms: 5_000,
        }
    }
}

impl RetryBudget {
    /// Compute the delay for `attempt` (0-indexed) and cap it.
    pub fn delay_for(&self, attempt: u32) -> u64 {
        let exp = 1u64 << attempt.min(30);
        let delay = self.base_ms.saturating_mul(exp);
        delay.min(self.cap_ms)
    }
}

/// A single classification rule: exact `code`, glob-like `stage` prefix, and
/// the action to try first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassificationRule {
    /// Error code to match. `None` matches any code.
    pub code: Option<u32>,
    /// Stage prefix. The rule applies if `stage` starts with this string.
    pub stage: &'static str,
    pub action: RecoveryAction,
}

/// Policy that maps errors to recovery actions and retry budgets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecoveryPolicy {
    rules: Vec<ClassificationRule>,
    budgets: [RetryBudget; 5],
}

impl RecoveryPolicy {
    /// Create a policy from rules and per-action retry budgets.
    ///
    /// `budgets` is indexed by `RecoveryAction` discriminants in declaration
    /// order: `RetrySame`, `RebuildStage`, `FallbackBackend`, `ReconnectSource`,
    /// `Fatal`.
    pub fn new(rules: Vec<ClassificationRule>, budgets: [RetryBudget; 5]) -> Self {
        Self { rules, budgets }
    }

    /// Sensible default policy for HTTP/WS-FLV, HLS and WebCodecs/MSE paths.
    pub fn default_player() -> Self {
        let rules = alloc::vec![
            // Network transient errors -> reconnect.
            ClassificationRule {
                code: None,
                stage: "network",
                action: RecoveryAction::ReconnectSource
            },
            // Decoder failures -> fallback backend, then rebuild, then reconnect.
            ClassificationRule {
                code: None,
                stage: "decoder",
                action: RecoveryAction::FallbackBackend
            },
            // Renderer failures -> rebuild (e.g. context lost).
            ClassificationRule {
                code: None,
                stage: "render",
                action: RecoveryAction::RebuildStage
            },
            // Resource limits are not retryable.
            ClassificationRule {
                code: None,
                stage: "limit",
                action: RecoveryAction::Fatal
            },
            // Catch-all for unknown backend errors: retry same, then fatal.
            ClassificationRule {
                code: None,
                stage: "backend",
                action: RecoveryAction::RetrySame
            },
        ];
        let budgets = [
            RetryBudget {
                max_attempts: 3,
                window_ms: 10_000,
                base_ms: 250,
                cap_ms: 2_000,
            }, // RetrySame
            RetryBudget {
                max_attempts: 2,
                window_ms: 15_000,
                base_ms: 500,
                cap_ms: 3_000,
            }, // RebuildStage
            RetryBudget {
                max_attempts: 2,
                window_ms: 20_000,
                base_ms: 250,
                cap_ms: 2_000,
            }, // FallbackBackend
            RetryBudget {
                max_attempts: 5,
                window_ms: 60_000,
                base_ms: 500,
                cap_ms: 8_000,
            }, // ReconnectSource
            RetryBudget {
                max_attempts: 0,
                window_ms: 0,
                base_ms: 0,
                cap_ms: 0,
            }, // Fatal
        ];
        Self { rules, budgets }
    }

    fn action_index(action: RecoveryAction) -> usize {
        match action {
            RecoveryAction::RetrySame => 0,
            RecoveryAction::RebuildStage => 1,
            RecoveryAction::FallbackBackend => 2,
            RecoveryAction::ReconnectSource => 3,
            RecoveryAction::Fatal => 4,
        }
    }

    /// Classify an error by matching the most specific rule.
    pub fn classify(&self, code: u32, stage: &str) -> RecoveryAction {
        let mut best: Option<&ClassificationRule> = None;
        for rule in &self.rules {
            let code_matches = rule.code.is_none_or(|c| c == code);
            let stage_matches = stage.starts_with(rule.stage);
            if code_matches && stage_matches {
                // Prefer longer (more specific) stage prefix; tie-break by
                // presence of an exact code match.
                let better = match best {
                    None => true,
                    Some(b) => {
                        let longer = rule.stage.len() > b.stage.len();
                        let exact = rule.code.is_some() && b.code.is_none();
                        longer || (rule.stage.len() == b.stage.len() && exact)
                    }
                };
                if better {
                    best = Some(rule);
                }
            }
        }
        best.map_or(RecoveryAction::Fatal, |r| r.action)
    }

    fn budget(&self, action: RecoveryAction) -> RetryBudget {
        self.budgets[Self::action_index(action)]
    }

    /// Largest `window_ms` across all budgets. Used to cap tracker history.
    pub fn max_window_ms(&self) -> u64 {
        self.budgets.iter().map(|b| b.window_ms).max().unwrap_or(0)
    }

    /// Decide whether `action` is still within budget.
    fn decide_action(
        &self,
        action: RecoveryAction,
        _code: u32,
        _stage: &str,
        now_ms: u64,
        attempts: &[u64],
    ) -> RecoveryDecision {
        if action == RecoveryAction::Fatal {
            return RecoveryDecision::Fatal;
        }
        let budget = self.budget(action);

        // Count attempts that fall inside the current window.
        let window_start = now_ms.saturating_sub(budget.window_ms);
        let in_window = attempts
            .iter()
            .filter(|&&t| t >= window_start && t <= now_ms)
            .count() as u32;

        if in_window >= budget.max_attempts {
            // Escalate through the action chain. If already at reconnect, become fatal.
            let next = match action {
                RecoveryAction::RetrySame => RecoveryAction::RebuildStage,
                RecoveryAction::RebuildStage => RecoveryAction::FallbackBackend,
                RecoveryAction::FallbackBackend => RecoveryAction::ReconnectSource,
                RecoveryAction::ReconnectSource => RecoveryAction::Fatal,
                RecoveryAction::Fatal => RecoveryAction::Fatal,
            };
            if next == RecoveryAction::Fatal {
                RecoveryDecision::Fatal
            } else {
                RecoveryDecision::Escalate { action: next }
            }
        } else {
            let attempts_so_far = attempts.len() as u32;
            let delay = budget.delay_for(attempts_so_far);
            let attempts_left = budget.max_attempts.saturating_sub(in_window);
            RecoveryDecision::Retry {
                action,
                delay_ms: delay,
                attempts_left,
            }
        }
    }

    /// Decide what to do for `(code, stage)` starting from the classified action
    /// and walking the escalation chain until a retryable action with remaining
    /// budget is found or the chain reaches `Fatal`.
    pub fn decide(
        &self,
        code: u32,
        stage: &str,
        now_ms: u64,
        attempts_for: &dyn Fn(RecoveryAction) -> Vec<u64>,
    ) -> RecoveryDecision {
        let mut action = self.classify(code, stage);
        for _ in 0..5 {
            let attempts = attempts_for(action);
            match self.decide_action(action, code, stage, now_ms, &attempts) {
                RecoveryDecision::Retry { .. } => {
                    return self.decide_action(action, code, stage, now_ms, &attempts);
                }
                RecoveryDecision::Fatal => return RecoveryDecision::Fatal,
                RecoveryDecision::Escalate { action: next } => action = next,
            }
        }
        RecoveryDecision::Fatal
    }
}

/// Tracks retry attempts per `(code, stage, action)` key.
#[derive(Debug, Clone, Default)]
pub struct RecoveryTracker {
    history: Vec<(u32, &'static str, RecoveryAction, u64)>,
}

impl RecoveryTracker {
    /// Record a recovery attempt at `now_ms`.
    pub fn record(&mut self, code: u32, stage: &'static str, action: RecoveryAction, now_ms: u64) {
        self.history.push((code, stage, action, now_ms));
    }

    /// Attempts for `(code, stage, action)` in chronological order.
    pub fn attempts(&self, code: u32, stage: &'static str, action: RecoveryAction) -> Vec<u64> {
        self.history
            .iter()
            .filter(|(c, s, a, _)| *c == code && *s == stage && *a == action)
            .map(|(_, _, _, t)| *t)
            .collect()
    }

    /// Prune entries older than `before_ms` to cap memory growth.
    pub fn prune(&mut self, before_ms: u64) {
        self.history.retain(|(_, _, _, t)| *t >= before_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_errors_default_to_reconnect() {
        let policy = RecoveryPolicy::default_player();
        assert_eq!(
            policy.classify(0, "network"),
            RecoveryAction::ReconnectSource
        );
    }

    #[test]
    fn decoder_errors_default_to_fallback() {
        let policy = RecoveryPolicy::default_player();
        assert_eq!(
            policy.classify(0, "decoder"),
            RecoveryAction::FallbackBackend
        );
    }

    #[test]
    fn exact_code_rule_beats_generic_stage_rule() {
        let rules = alloc::vec![
            ClassificationRule {
                code: Some(42),
                stage: "backend",
                action: RecoveryAction::ReconnectSource
            },
            ClassificationRule {
                code: None,
                stage: "backend",
                action: RecoveryAction::RetrySame
            },
        ];
        let policy = RecoveryPolicy::new(rules, [RetryBudget::default(); 5]);
        assert_eq!(
            policy.classify(42, "backend"),
            RecoveryAction::ReconnectSource
        );
        assert_eq!(policy.classify(99, "backend"), RecoveryAction::RetrySame);
    }

    #[test]
    fn retry_budgets_grow_then_cap() {
        let budget = RetryBudget {
            max_attempts: 3,
            window_ms: 10_000,
            base_ms: 100,
            cap_ms: 500,
        };
        assert_eq!(budget.delay_for(0), 100);
        assert_eq!(budget.delay_for(1), 200);
        assert_eq!(budget.delay_for(2), 400);
        assert_eq!(budget.delay_for(3), 500);
        assert_eq!(budget.delay_for(10), 500);
    }

    #[test]
    fn retry_within_budget_allows_retry() {
        let policy = RecoveryPolicy::default_player();
        let attempts = alloc::vec![0u64, 100, 300];
        let decision = policy.decide(1, "network", 500, &|_| attempts.clone());
        assert!(matches!(
            decision,
            RecoveryDecision::Retry {
                action: RecoveryAction::ReconnectSource,
                ..
            }
        ));
    }

    #[test]
    fn exhausted_window_escalates() {
        let policy = RecoveryPolicy::default_player();
        // Five reconnect attempts inside the 60s window -> escalate to fatal.
        let attempts = alloc::vec![1000u64, 2000, 3000, 4000, 5000];
        let decision = policy.decide(1, "network", 6000, &|_| attempts.clone());
        assert_eq!(decision, RecoveryDecision::Fatal);
    }

    #[test]
    fn escalation_falls_back_to_next_action_budget() {
        let policy = RecoveryPolicy::default_player();
        let mut tracker = RecoveryTracker::default();
        // Burn the FallbackBackend budget for decoder errors.
        for t in 0..2 {
            tracker.record(1, "decoder", RecoveryAction::FallbackBackend, t * 1000);
        }
        // The next decision should escalate to ReconnectSource and find a fresh budget.
        let decision = policy.decide(1, "decoder", 3000, &|a| tracker.attempts(1, "decoder", a));
        assert!(matches!(
            decision,
            RecoveryDecision::Retry {
                action: RecoveryAction::ReconnectSource,
                ..
            }
        ));
    }

    #[test]
    fn tracker_records_and_prunes() {
        let mut tracker = RecoveryTracker::default();
        tracker.record(10, "decoder", RecoveryAction::RetrySame, 1000);
        tracker.record(10, "decoder", RecoveryAction::RetrySame, 2000);
        tracker.record(20, "render", RecoveryAction::RebuildStage, 1500);
        assert_eq!(
            tracker.attempts(10, "decoder", RecoveryAction::RetrySame),
            alloc::vec![1000, 2000]
        );
        assert_eq!(
            tracker.attempts(20, "render", RecoveryAction::RebuildStage),
            alloc::vec![1500]
        );

        // Prune entries older than 1200ms. Both remaining entries are newer.
        tracker.prune(1200);
        assert_eq!(
            tracker.attempts(10, "decoder", RecoveryAction::RetrySame),
            alloc::vec![2000]
        );
        assert_eq!(
            tracker.attempts(20, "render", RecoveryAction::RebuildStage),
            alloc::vec![1500]
        );
    }

    #[test]
    fn fatal_action_decision_is_fatal() {
        let policy = RecoveryPolicy::default_player();
        let decision = policy.decide(0, "limit", 0, &|_| alloc::vec![]);
        assert_eq!(decision, RecoveryDecision::Fatal);
    }
}

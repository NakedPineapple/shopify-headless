//! Circuit breaker for scheduler workflows.
//!
//! Tracks consecutive failures per workflow. After a configurable number of
//! consecutive failures (default: 5), the workflow is "tripped" and paused
//! for a cooldown period (default: 10 minutes). After the cooldown, the
//! breaker allows one probe attempt — if it succeeds, the breaker resets;
//! if it fails, the cooldown restarts.
//!
//! Each scheduler loop is independent: one workflow's breaker tripping
//! does not affect other workflows.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Default number of consecutive failures before tripping.
const DEFAULT_FAILURE_THRESHOLD: u32 = 5;

/// Default cooldown period after tripping (10 minutes).
const DEFAULT_COOLDOWN: Duration = Duration::from_secs(600);

/// Circuit breaker state for a single workflow.
#[derive(Default)]
struct WorkflowBreaker {
    /// Number of consecutive failures.
    consecutive_failures: u32,
    /// Time when the breaker was tripped (None if not tripped).
    tripped_at: Option<Instant>,
}

/// Circuit breaker manager for all scheduler workflows.
pub struct CircuitBreaker {
    breakers: HashMap<&'static str, WorkflowBreaker>,
    failure_threshold: u32,
    cooldown: Duration,
}

impl CircuitBreaker {
    /// Create a new circuit breaker manager with default settings.
    pub fn new() -> Self {
        Self {
            breakers: HashMap::new(),
            failure_threshold: DEFAULT_FAILURE_THRESHOLD,
            cooldown: DEFAULT_COOLDOWN,
        }
    }

    /// Check if a workflow is allowed to run.
    ///
    /// Returns `true` if the workflow should execute, `false` if it's
    /// currently in cooldown.
    pub fn is_allowed(&self, workflow: &'static str) -> bool {
        let Some(breaker) = self.breakers.get(workflow) else {
            return true;
        };

        let Some(tripped_at) = breaker.tripped_at else {
            return true;
        };

        // Allow probe after cooldown expires
        tripped_at.elapsed() >= self.cooldown
    }

    /// Record a successful execution. Resets the consecutive failure count
    /// and clears any tripped state.
    pub fn record_success(&mut self, workflow: &'static str) {
        let breaker = self.breakers.entry(workflow).or_default();
        breaker.consecutive_failures = 0;
        breaker.tripped_at = None;
    }

    /// Record a failed execution. Increments the consecutive failure count
    /// and trips the breaker if the threshold is reached.
    ///
    /// Returns `true` if the breaker just tripped (newly exceeded threshold).
    pub fn record_failure(&mut self, workflow: &'static str) -> bool {
        let breaker = self.breakers.entry(workflow).or_default();
        breaker.consecutive_failures += 1;

        if breaker.consecutive_failures >= self.failure_threshold && breaker.tripped_at.is_none() {
            breaker.tripped_at = Some(Instant::now());
            return true;
        }

        // If already tripped and probe failed, restart cooldown
        if breaker.tripped_at.is_some() {
            breaker.tripped_at = Some(Instant::now());
        }

        false
    }

    /// Get the number of consecutive failures for a workflow.
    pub fn failure_count(&self, workflow: &'static str) -> u32 {
        self.breakers
            .get(workflow)
            .map_or(0, |b| b.consecutive_failures)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_allows_initially() {
        let cb = CircuitBreaker::new();
        assert!(cb.is_allowed("test_workflow"));
    }

    #[test]
    fn test_allows_after_few_failures() {
        let mut cb = CircuitBreaker::new();
        for _ in 0..4 {
            let tripped = cb.record_failure("test_workflow");
            assert!(!tripped);
        }
        assert!(cb.is_allowed("test_workflow"));
    }

    #[test]
    fn test_trips_at_threshold() {
        let mut cb = CircuitBreaker::new();
        for _ in 0..4 {
            cb.record_failure("test_workflow");
        }
        let tripped = cb.record_failure("test_workflow");
        assert!(tripped);
        assert!(!cb.is_allowed("test_workflow"));
    }

    #[test]
    fn test_resets_on_success() {
        let mut cb = CircuitBreaker::new();
        for _ in 0..5 {
            cb.record_failure("test_workflow");
        }
        assert!(!cb.is_allowed("test_workflow"));

        cb.record_success("test_workflow");
        assert!(cb.is_allowed("test_workflow"));
        assert_eq!(cb.failure_count("test_workflow"), 0);
    }

    #[test]
    fn test_independent_workflows() {
        let mut cb = CircuitBreaker::new();
        for _ in 0..5 {
            cb.record_failure("workflow_a");
        }
        assert!(!cb.is_allowed("workflow_a"));
        assert!(cb.is_allowed("workflow_b"));
    }

    #[test]
    fn test_allows_after_cooldown() {
        let mut cb = CircuitBreaker {
            breakers: HashMap::new(),
            failure_threshold: DEFAULT_FAILURE_THRESHOLD,
            cooldown: Duration::from_millis(1), // Very short for testing
        };

        for _ in 0..5 {
            cb.record_failure("test_workflow");
        }
        assert!(!cb.is_allowed("test_workflow"));

        // Wait for cooldown
        std::thread::sleep(Duration::from_millis(5));
        assert!(cb.is_allowed("test_workflow"));
    }
}

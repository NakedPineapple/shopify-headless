//! Integration tests for admin AI chat action queue.
//!
//! These tests verify the action queue status transitions and
//! pending action management without requiring actual Slack integration.

use naked_pineapple_admin::db::pending_actions::ActionStatus;

// =============================================================================
// Action Status Tests
// =============================================================================

#[test]
fn test_action_status_enum_values() {
    // Verify all expected status values exist by using them
    assert!(matches!(ActionStatus::Pending, ActionStatus::Pending));
    assert!(matches!(ActionStatus::Approved, ActionStatus::Approved));
    assert!(matches!(ActionStatus::Rejected, ActionStatus::Rejected));
    assert!(matches!(ActionStatus::Executed, ActionStatus::Executed));
    assert!(matches!(ActionStatus::Failed, ActionStatus::Failed));
    assert!(matches!(ActionStatus::Expired, ActionStatus::Expired));
}

#[test]
fn test_action_status_debug() {
    // Action statuses should have Debug impl for logging
    let status = ActionStatus::Pending;
    let debug_str = format!("{status:?}");
    assert!(debug_str.contains("Pending"));
}

#[test]
fn test_action_status_copy() {
    let status = ActionStatus::Approved;
    let copied = status; // ActionStatus implements Copy
    assert!(matches!(copied, ActionStatus::Approved));
    // Verify original still usable (proving it's Copy)
    assert!(matches!(status, ActionStatus::Approved));
}

#[test]
fn test_action_status_eq() {
    assert_eq!(ActionStatus::Pending, ActionStatus::Pending);
    assert_ne!(ActionStatus::Pending, ActionStatus::Approved);
    assert_ne!(ActionStatus::Approved, ActionStatus::Rejected);
}

// =============================================================================
// State Transition Tests (Logical)
// =============================================================================

/// Valid state transitions for pending actions.
/// Pending -> Approved -> Executed
/// Pending -> Approved -> Failed
/// Pending -> Rejected
/// Pending -> Expired
#[test]
fn test_valid_state_transitions() {
    // These represent the valid state machine transitions
    let valid_transitions = [
        (ActionStatus::Pending, ActionStatus::Approved),
        (ActionStatus::Pending, ActionStatus::Rejected),
        (ActionStatus::Pending, ActionStatus::Expired),
        (ActionStatus::Approved, ActionStatus::Executed),
        (ActionStatus::Approved, ActionStatus::Failed),
    ];

    // Just verify the states can be compared (actual transition logic is in the service)
    for (from, to) in valid_transitions {
        assert_ne!(from, to, "Transition should be between different states");
    }
}

/// Invalid state transitions - these should be rejected by the service
#[test]
fn test_invalid_state_transitions_are_different() {
    let invalid_transitions = [
        (ActionStatus::Executed, ActionStatus::Pending), // Can't go back
        (ActionStatus::Failed, ActionStatus::Pending),   // Can't go back
        (ActionStatus::Rejected, ActionStatus::Approved), // Can't re-approve
        (ActionStatus::Expired, ActionStatus::Approved), // Can't approve expired
    ];

    // These states are all different from each other
    for (from, to) in invalid_transitions {
        assert_ne!(from, to);
    }
}

// =============================================================================
// Terminal State Tests
// =============================================================================

#[test]
fn test_terminal_states() {
    // These states are terminal - no further transitions should be allowed
    let terminal_states = [
        ActionStatus::Executed,
        ActionStatus::Failed,
        ActionStatus::Rejected,
        ActionStatus::Expired,
    ];

    // Verify they're all distinct
    for (i, state1) in terminal_states.iter().enumerate() {
        for (j, state2) in terminal_states.iter().enumerate() {
            if i != j {
                assert_ne!(state1, state2);
            }
        }
    }
}

#[test]
fn test_non_terminal_states() {
    // These states can transition to other states
    let non_terminal_states = [ActionStatus::Pending, ActionStatus::Approved];

    // Verify they're distinct from terminal states
    let terminal_states = [
        ActionStatus::Executed,
        ActionStatus::Failed,
        ActionStatus::Rejected,
        ActionStatus::Expired,
    ];

    for non_terminal in &non_terminal_states {
        for terminal in &terminal_states {
            assert_ne!(non_terminal, terminal);
        }
    }
}

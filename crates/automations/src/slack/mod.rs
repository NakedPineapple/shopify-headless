//! Slack integration for email triage.
//!
//! Provides message builders for email review notifications and a webhook
//! handler for Slack interactive callbacks (approve/reject buttons).

pub mod messages;
pub mod webhook;

//! Slack integration for AI chat confirmations.
//!
//! This module provides:
//! - [`SlackClient`] for sending and updating messages
//! - Block Kit types for building rich messages
//! - Message builders for confirmation flows
//! - Webhook signature verification
//!
//! # Flow
//!
//! 1. When a write operation is requested, a confirmation message is sent to Slack
//! 2. Admin clicks Approve or Reject button
//! 3. Webhook handler receives interaction, verifies signature
//! 4. Action is executed or rejected
//! 5. Message is updated with result

mod client;
mod error;
mod messages;
mod types;

pub use client::SlackClient;
pub use error::SlackError;
pub use messages::{
    build_approved_message, build_confirmation_message, build_error_message,
    build_rejected_message, build_timeout_message,
};
pub use types::{
    ActionElement, Block, ButtonStyle, ContextElement, InteractionAction, InteractionPayload,
    InteractionUser, PlainText, PostMessageResponse, Text, UpdateMessageResponse,
};

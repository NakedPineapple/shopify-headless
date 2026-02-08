//! Slack integration for notifications and approval workflows.
//!
//! Provides:
//! - [`SlackClient`] for sending and updating messages
//! - Block Kit types for building rich messages
//! - Message builders for confirmation flows
//! - Webhook signature verification

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

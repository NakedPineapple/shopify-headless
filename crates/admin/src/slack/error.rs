//! Slack-related errors.

use thiserror::Error;

/// Errors that can occur when interacting with Slack.
#[derive(Debug, Error)]
pub enum SlackError {
    /// HTTP request failed.
    #[error("Slack request failed: {0}")]
    Request(String),

    /// Failed to parse response.
    #[error("Slack response error: {0}")]
    Response(String),

    /// Slack API returned an error.
    #[error("Slack API error: {0}")]
    Api(String),

    /// Invalid webhook signature.
    #[error("Invalid Slack signature: {0}")]
    InvalidSignature(String),

    /// Failed to parse interaction payload.
    #[error("Invalid interaction payload: {0}")]
    InvalidPayload(String),

    /// Configuration error.
    #[error("Slack configuration error: {0}")]
    Config(String),
}

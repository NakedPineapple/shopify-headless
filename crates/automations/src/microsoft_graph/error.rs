//! Microsoft Graph API error types.

use thiserror::Error;

/// Errors that can occur when interacting with the Microsoft Graph API.
#[derive(Debug, Error)]
pub enum M365Error {
    /// `OAuth2` token acquisition failed.
    #[error("authentication failed: {0}")]
    Authentication(String),

    /// HTTP request failed.
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),

    /// Graph API returned an error response.
    #[error("Graph API error {status}: {message}")]
    Api {
        status: u16,
        message: String,
        error_code: Option<String>,
    },

    /// Failed to parse response.
    #[error("failed to parse response: {0}")]
    Parse(String),
}

//! Error types for the Judge.me API client.

use thiserror::Error;

/// Errors that can occur when interacting with the Judge.me API.
#[derive(Debug, Error)]
pub enum JudgemeError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Judge.me API returned an error response.
    #[error("API error (status {status}): {message}")]
    Api {
        /// HTTP status code from the API.
        status: u16,
        /// Error message or body.
        message: String,
    },

    /// No Judge.me product found for the given Shopify external ID.
    #[error("no Judge.me product found for Shopify external ID {0}")]
    ProductNotFound(i64),

    /// Failed to parse API response.
    #[error("parse error: {0}")]
    Parse(String),
}

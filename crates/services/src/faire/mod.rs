//! Faire Brand API client (wholesale marketplace).
//!
//! Provides access to the Faire Brand API v2 for managing wholesale
//! orders, product catalog, returns, and payouts.
//!
//! # API Reference
//!
//! - Base URL: `https://www.faire.com/api/v2`
//! - Auth: `X-FAIRE-ACCESS-TOKEN` header
//! - Rate limiting: 100 requests per 10 seconds

mod catalog;
mod client;
mod orders;
mod payouts;
mod returns;
mod types;

pub use client::FaireClient;
pub use types::*;

use thiserror::Error;

/// Errors that can occur when interacting with the Faire API.
#[derive(Debug, Error)]
pub enum FaireError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Faire API returned an error response.
    #[error("Faire API error: {status} - {message}")]
    Api { status: u16, message: String },

    /// Rate limited (HTTP 429).
    #[error("Rate limited, retry after {0} seconds")]
    RateLimited(u64),

    /// Unauthorized (invalid token).
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Failed to parse response.
    #[error("Parse error: {0}")]
    Parse(String),

    /// Resource not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Client not configured.
    #[error("Faire client not configured")]
    NotConfigured,
}

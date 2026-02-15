//! Pinterest API client (Catalog Sync + Conversions API).
//!
//! Provides access to Pinterest API v5 for managing product catalogs
//! and reporting conversion events for attribution.
//!
//! # API Reference
//!
//! - Base URL: `https://api.pinterest.com/v5`
//! - Auth: OAuth 2.0 Bearer Token
//! - Rate limiting: 1000 requests/min (general), 5000/min (Conversions API)

mod auth;
mod catalog;
mod client;
mod conversions;
mod types;

pub use client::PinterestClient;
pub use types::*;

use thiserror::Error;

/// Errors that can occur when interacting with the Pinterest API.
#[derive(Debug, Error)]
pub enum PinterestError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Pinterest API returned an error response.
    #[error("Pinterest API error: {status} - {message}")]
    Api { status: u16, message: String },

    /// Rate limited (HTTP 429).
    #[error("Rate limited, retry after {0} seconds")]
    RateLimited(u64),

    /// Unauthorized (invalid or expired token).
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Token refresh failed.
    #[error("Token refresh error: {0}")]
    TokenRefresh(String),

    /// Failed to parse response.
    #[error("Parse error: {0}")]
    Parse(String),

    /// Resource not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Client not configured.
    #[error("Pinterest client not configured")]
    NotConfigured,
}

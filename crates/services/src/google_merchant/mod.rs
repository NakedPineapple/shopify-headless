//! Google Merchant Center API client (Content API for Shopping).
//!
//! Provides access to Google Merchant Center for managing product catalogs
//! used in Google Shopping and `YouTube` Shopping.
//!
//! # API Reference
//!
//! - Base URL: `https://shoppingcontent.googleapis.com/content/v2.1`
//! - Auth: OAuth 2.0 Bearer Token
//! - Rate limiting: Varies by endpoint

mod auth;
mod catalog;
mod client;
mod types;

pub use client::GoogleMerchantClient;
pub use types::*;

use thiserror::Error;

/// Errors that can occur when interacting with the Google Merchant Center API.
#[derive(Debug, Error)]
pub enum GoogleMerchantError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Google API returned an error response.
    #[error("Google API error: {status} - {message}")]
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
    #[error("Google Merchant client not configured")]
    NotConfigured,
}

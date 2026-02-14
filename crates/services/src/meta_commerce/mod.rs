//! Meta Commerce API client (Facebook Shop + Instagram Shopping).
//!
//! Provides access to Meta's Commerce API via the Graph API for managing
//! catalog products and orders across Facebook and Instagram sales channels.
//!
//! # API Reference
//!
//! - Base URL: `https://graph.facebook.com/v21.0`
//! - Auth: Page Access Token (OAuth 2.0 bearer token)
//! - Rate limiting: `x-business-use-case-usage` header

mod auth;
mod catalog;
mod client;
mod orders;
mod shop;
mod types;

pub use client::MetaCommerceClient;
pub use types::*;

use thiserror::Error;

/// Errors that can occur when interacting with the Meta Commerce API.
#[derive(Debug, Error)]
pub enum MetaCommerceError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Graph API returned an error response.
    #[error("Graph API error: {status} - {message}")]
    Api { status: u16, message: String },

    /// Rate limited (HTTP 429 or usage threshold exceeded).
    #[error("Rate limited, retry after {0} seconds")]
    RateLimited(u64),

    /// Unauthorized (invalid or expired token).
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Token exchange failed.
    #[error("Token exchange error: {0}")]
    TokenExchange(String),

    /// Failed to parse response.
    #[error("Parse error: {0}")]
    Parse(String),

    /// Resource not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Client not configured.
    #[error("Meta Commerce client not configured")]
    NotConfigured,
}

//! TikTok Shop Open API v2 client.
//!
//! Provides access to TikTok Shop's Open API for managing products, orders,
//! returns, settlements, and shop performance across TikTok's commerce platform.
//!
//! # API Reference
//!
//! - Base URL: `https://open-api.tiktokglobalshop.com`
//! - Auth: OAuth 2.0 with HMAC-SHA256 request signing
//! - Rate limits: 600/endpoint, 1000/day

mod auth;
mod catalog;
mod client;
mod finance;
mod orders;
mod returns;
mod shop;
mod signing;
mod types;

pub use client::TikTokShopClient;
pub use types::*;

use thiserror::Error;

/// Errors that can occur when interacting with the TikTok Shop API.
#[derive(Debug, Error)]
pub enum TikTokShopError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// TikTok API returned an error response.
    #[error("API error: code={code}, message={message}")]
    Api { code: i32, message: String },

    /// Rate limited (HTTP 429 or usage threshold exceeded).
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

    /// Request signing failed.
    #[error("Signing error: {0}")]
    Signing(String),

    /// Client not configured.
    #[error("TikTok Shop client not configured")]
    NotConfigured,
}

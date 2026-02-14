//! Amazon Selling Partner API (SP-API) client.
//!
//! Provides access to Amazon's SP-API for managing catalog, inventory,
//! orders, and pricing data. Requires dual-layer auth: Login with Amazon (LWA)
//! OAuth tokens + AWS Signature Version 4 request signing.
//!
//! # API Reference
//!
//! - Base URL: `https://sellingpartnerapi-na.amazon.com`
//! - LWA Token: `https://api.amazon.com/auth/o2/token`
//! - Auth: LWA access token + AWS `SigV4` on every request

mod auth;
mod catalog;
mod client;
mod inventory;
mod listings;
mod orders;
mod pricing;
mod reports;
mod types;

pub use client::AmazonSpClient;
pub use inventory::InventorySummariesPage;
pub use orders::OrdersPage;
pub use pricing::PricingResult;
pub use reports::{CreateReportRequest, Report, ReportDocument};
pub use types::*;

use thiserror::Error;

/// Errors that can occur when interacting with the Amazon SP-API.
#[derive(Debug, Error)]
pub enum AmazonSpError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// SP-API returned an error response.
    #[error("SP-API error: {status} - {message}")]
    Api { status: u16, message: String },

    /// Rate limited (HTTP 429).
    #[error("Rate limited, retry after {0} seconds")]
    RateLimited(u64),

    /// Unauthorized (invalid or expired credentials).
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// LWA token exchange failed.
    #[error("LWA token error: {0}")]
    TokenExchange(String),

    /// AWS `SigV4` signing failed.
    #[error("Request signing error: {0}")]
    Signing(String),

    /// Failed to parse response.
    #[error("Parse error: {0}")]
    Parse(String),

    /// Resource not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Client not configured.
    #[error("Amazon SP-API client not configured")]
    NotConfigured,
}

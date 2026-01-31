//! `ShipHero` warehouse management API client.
//!
//! Provides read-only access to `ShipHero`'s GraphQL API for viewing warehouse data
//! including orders awaiting fulfillment, shipment history, and inventory levels.
//!
//! # Architecture
//!
//! - Uses two-layer authentication: email/password → JWT → API
//! - JWT tokens stored in database, loaded at startup
//! - Type-safe GraphQL queries via `graphql-client` crate
//! - Read-only integration (native `ShipHero`↔Shopify sync handles data flow)
//!
//! # Security
//!
//! `ShipHero` credentials can only be configured by `super_admin` users via the
//! settings page. The integration is optional—if not configured, the admin
//! panel works normally without `ShipHero` features.

pub mod auth;
pub mod client;
pub mod orders;
pub mod queries;

pub use client::ShipHeroClient;
pub use orders::*;

use thiserror::Error;

/// Errors that can occur when interacting with the `ShipHero` API.
#[derive(Debug, Error)]
pub enum ShipHeroError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// GraphQL query returned errors.
    #[error("GraphQL errors: {}", format_graphql_errors(.0))]
    GraphQL(Vec<GraphQLError>),

    /// JSON parsing failed.
    #[error("JSON parse error: {0}")]
    Parse(#[from] serde_json::Error),

    /// Resource not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Rate limited by `ShipHero`.
    #[error("Rate limited, retry after {0} seconds")]
    RateLimited(u64),

    /// Authentication failed (invalid email/password).
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Access token expired.
    #[error("Access token expired")]
    TokenExpired,

    /// No valid access token available (authentication required).
    #[error("No access token - ShipHero authentication required")]
    NoAccessToken,

    /// Database error during credential operations.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// A GraphQL error returned by the `ShipHero` API.
#[derive(Debug, Clone)]
pub struct GraphQLError {
    /// Error message.
    pub message: String,
    /// Source locations in the query.
    pub locations: Vec<GraphQLErrorLocation>,
    /// Path to the error in the response.
    pub path: Vec<serde_json::Value>,
}

/// Location in a GraphQL query where an error occurred.
#[derive(Debug, Clone)]
pub struct GraphQLErrorLocation {
    /// Line number (1-indexed).
    pub line: i64,
    /// Column number (1-indexed).
    pub column: i64,
}

fn format_graphql_errors(errors: &[GraphQLError]) -> String {
    errors
        .iter()
        .map(|e| e.message.clone())
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shiphero_error_display() {
        let err = ShipHeroError::NotFound("order-123".to_string());
        assert_eq!(err.to_string(), "Not found: order-123");
    }

    #[test]
    fn test_graphql_error_formatting() {
        let errors = vec![
            GraphQLError {
                message: "Field not found".to_string(),
                locations: vec![],
                path: vec![],
            },
            GraphQLError {
                message: "Invalid ID".to_string(),
                locations: vec![],
                path: vec![],
            },
        ];
        let err = ShipHeroError::GraphQL(errors);
        assert_eq!(
            err.to_string(),
            "GraphQL errors: Field not found; Invalid ID"
        );
    }

    #[test]
    fn test_rate_limited_error() {
        let err = ShipHeroError::RateLimited(60);
        assert_eq!(err.to_string(), "Rate limited, retry after 60 seconds");
    }

    #[test]
    fn test_token_expired_error() {
        let err = ShipHeroError::TokenExpired;
        assert_eq!(err.to_string(), "Access token expired");
    }

    #[test]
    fn test_authentication_failed_error() {
        let err = ShipHeroError::AuthenticationFailed("Invalid credentials".to_string());
        assert_eq!(
            err.to_string(),
            "Authentication failed: Invalid credentials"
        );
    }
}

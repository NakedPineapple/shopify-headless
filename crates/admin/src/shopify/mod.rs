//! Shopify Admin API client (HIGH PRIVILEGE - Tailscale only).
//!
//! # Security
//!
//! **CRITICAL: This crate contains the high-privilege Shopify Admin API token.**
//!
//! It should ONLY run on Tailscale-protected infrastructure with MDM verification.
//! The Admin API has full access to:
//! - Products, variants, inventory
//! - Orders, fulfillments, refunds
//! - Customers, customer data
//! - Discounts, price rules
//! - Shop settings
//!
//! # Architecture
//!
//! - Uses `graphql-client` crate for type-safe GraphQL queries
//! - Direct API calls to Shopify (no local database sync)
//! - Rate limiting handled automatically
//!
//! # Example
//!
//! ```rust,ignore
//! use naked_pineapple_admin::shopify::AdminClient;
//!
//! let client = AdminClient::new(&config.shopify);
//!
//! // Get products
//! let products = client.get_products(10, None, None).await?;
//!
//! // Get a specific order
//! let order = client.get_order("gid://shopify/Order/123").await?;
//!
//! // Adjust inventory
//! client.adjust_inventory(
//!     "gid://shopify/InventoryItem/123",
//!     "gid://shopify/Location/456",
//!     -1, // decrease by 1
//! ).await?;
//! ```

// Allow dead code during incremental development
#![allow(dead_code)]
#![allow(unused_imports)]

mod admin;
pub mod types;

pub use admin::AdminClient;
pub use types::*;

use thiserror::Error;

/// Errors that can occur when interacting with Shopify Admin API.
#[derive(Debug, Error)]
pub enum AdminShopifyError {
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

    /// Rate limited by Shopify.
    #[error("Rate limited, retry after {0} seconds")]
    RateLimited(u64),

    /// Authentication/authorization failed.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// User error from mutation (e.g., invalid input).
    #[error("User error: {0}")]
    UserError(String),
}

/// A GraphQL error returned by the Shopify Admin API.
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
    fn test_admin_shopify_error_display() {
        let err = AdminShopifyError::NotFound("order-123".to_string());
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
        let err = AdminShopifyError::GraphQL(errors);
        assert_eq!(
            err.to_string(),
            "GraphQL errors: Field not found; Invalid ID"
        );
    }

    #[test]
    fn test_rate_limited_error() {
        let err = AdminShopifyError::RateLimited(60);
        assert_eq!(err.to_string(), "Rate limited, retry after 60 seconds");
    }

    #[test]
    fn test_unauthorized_error() {
        let err = AdminShopifyError::Unauthorized("Invalid token".to_string());
        assert_eq!(err.to_string(), "Unauthorized: Invalid token");
    }

    #[test]
    fn test_user_error() {
        let err = AdminShopifyError::UserError("Invalid quantity".to_string());
        assert_eq!(err.to_string(), "User error: Invalid quantity");
    }
}

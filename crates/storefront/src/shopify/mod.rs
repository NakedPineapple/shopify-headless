//! Shopify Storefront and Customer Account API clients.
//!
//! # Architecture
//!
//! - Uses `graphql-client` crate for type-safe GraphQL queries
//! - Shopify is source of truth - NO local sync, direct API calls
//! - In-memory caching via `moka` for API responses (5 minute TTL)
//!
//! # APIs
//!
//! ## Storefront API
//! - Products, collections, cart operations
//! - Public access token for client-side operations
//! - Private access token for server-side operations
//!
//! ## Customer Account API
//! - OAuth authentication flow
//! - Customer data, order history
//! - Requires customer consent
//!
//! # Example
//!
//! ```rust,ignore
//! use naked_pineapple_storefront::shopify::StorefrontClient;
//!
//! let client = StorefrontClient::new(&config.shopify);
//!
//! // Get a product
//! let product = client.get_product_by_handle("my-product").await?;
//!
//! // Create a cart and add items
//! let cart = client.create_cart(None, None).await?;
//! let cart = client.add_to_cart(&cart.id, vec![CartLineInput {
//!     merchandise_id: product.variants[0].id.clone(),
//!     quantity: 1,
//!     attributes: None,
//!     selling_plan_id: None,
//! }]).await?;
//! ```

// Allow dead code during incremental development
#![allow(dead_code)]
#![allow(unused_imports)]

mod storefront;
pub mod types;

pub use storefront::StorefrontClient;
pub use types::*;

use thiserror::Error;

/// Errors that can occur when interacting with Shopify APIs.
#[derive(Debug, Error)]
pub enum ShopifyError {
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

    /// User error from mutation (e.g., invalid input).
    #[error("User error: {0}")]
    UserError(String),
}

/// A GraphQL error returned by the Shopify API.
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
    if errors.is_empty() {
        return "(no error details provided)".to_string();
    }

    errors
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut parts = Vec::new();

            // Include message if present
            if !e.message.is_empty() {
                parts.push(e.message.clone());
            }

            // Include path if present
            if !e.path.is_empty() {
                let path_str = e
                    .path
                    .iter()
                    .map(|p| match p {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(".");
                parts.push(format!("path: {path_str}"));
            }

            // Include location if present
            if !e.locations.is_empty() {
                let loc = &e.locations[0];
                parts.push(format!("at line {}:{}", loc.line, loc.column));
            }

            if parts.is_empty() {
                format!("[error {}]: (no details)", i + 1)
            } else {
                parts.join(" ")
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shopify_error_display() {
        let err = ShopifyError::NotFound("product-123".to_string());
        assert_eq!(err.to_string(), "Not found: product-123");
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
        let err = ShopifyError::GraphQL(errors);
        assert_eq!(
            err.to_string(),
            "GraphQL errors: Field not found; Invalid ID"
        );
    }

    #[test]
    fn test_graphql_error_empty_messages() {
        // Test with empty messages but with path info
        let errors = vec![GraphQLError {
            message: String::new(),
            locations: vec![GraphQLErrorLocation { line: 5, column: 10 }],
            path: vec![
                serde_json::Value::String("products".to_string()),
                serde_json::Value::Number(0.into()),
            ],
        }];
        let err = ShopifyError::GraphQL(errors);
        assert_eq!(
            err.to_string(),
            "GraphQL errors: path: products.0 at line 5:10"
        );
    }

    #[test]
    fn test_graphql_error_no_details() {
        let errors = vec![GraphQLError {
            message: String::new(),
            locations: vec![],
            path: vec![],
        }];
        let err = ShopifyError::GraphQL(errors);
        assert_eq!(err.to_string(), "GraphQL errors: [error 1]: (no details)");
    }

    #[test]
    fn test_graphql_error_empty_vec() {
        let err = ShopifyError::GraphQL(vec![]);
        assert_eq!(
            err.to_string(),
            "GraphQL errors: (no error details provided)"
        );
    }

    #[test]
    fn test_rate_limited_error() {
        let err = ShopifyError::RateLimited(60);
        assert_eq!(err.to_string(), "Rate limited, retry after 60 seconds");
    }
}

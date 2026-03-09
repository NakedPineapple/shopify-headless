//! Pineapple Skin Co. Core - Shared types library.
//!
//! This crate provides common types used across all Pineapple Skin Co. components:
//! - `storefront` - Public-facing e-commerce site
//! - `admin` - Internal administration panel (Tailscale-only)
//! - `cli` - Command-line tools for migrations and management
//!
//! # Architecture
//!
//! The core crate contains only types and traits - no I/O, no database access,
//! no HTTP clients. This keeps it lightweight and allows it to be used anywhere.
//!
//! # Modules
//!
//! - [`types`] - Newtype wrappers for type-safe IDs, prices, emails, and statuses

#![cfg_attr(not(test), forbid(unsafe_code))]

pub mod types;

pub use types::*;

/// Extract the numeric ID from a Shopify GID string.
///
/// Shopify uses Global IDs like `"gid://shopify/Product/123456789"`. This
/// function extracts the trailing numeric portion.
///
/// # Examples
///
/// ```
/// use naked_pineapple_core::extract_shopify_numeric_id;
///
/// assert_eq!(extract_shopify_numeric_id("gid://shopify/Product/123"), Some(123));
/// assert_eq!(extract_shopify_numeric_id("gid://shopify/ProductVariant/456"), Some(456));
/// assert_eq!(extract_shopify_numeric_id("not-a-gid"), None);
/// ```
#[must_use]
pub fn extract_shopify_numeric_id(gid: &str) -> Option<i64> {
    gid.rsplit('/').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_product_gid() {
        assert_eq!(
            extract_shopify_numeric_id("gid://shopify/Product/123456789"),
            Some(123_456_789)
        );
    }

    #[test]
    fn extract_variant_gid() {
        assert_eq!(
            extract_shopify_numeric_id("gid://shopify/ProductVariant/987654321"),
            Some(987_654_321)
        );
    }

    #[test]
    fn extract_collection_gid() {
        assert_eq!(
            extract_shopify_numeric_id("gid://shopify/Collection/42"),
            Some(42)
        );
    }

    #[test]
    fn returns_none_for_non_numeric_trailing() {
        assert_eq!(
            extract_shopify_numeric_id("gid://shopify/Product/abc"),
            None
        );
    }

    #[test]
    fn returns_none_for_empty_string() {
        assert_eq!(extract_shopify_numeric_id(""), None);
    }

    #[test]
    fn plain_number_parses() {
        assert_eq!(extract_shopify_numeric_id("999"), Some(999));
    }

    #[test]
    fn returns_none_for_no_numeric_segment() {
        assert_eq!(extract_shopify_numeric_id("not-a-gid"), None);
    }
}

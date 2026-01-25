//! Custom scalar types for Shopify GraphQL API.

/// Custom scalar for DateTime (Shopify returns ISO 8601 strings).
pub type DateTime = String;

/// Custom scalar for Decimal (Shopify returns decimal strings).
pub type Decimal = String;

/// Custom scalar for URL (Shopify returns URL strings).
pub type URL = String;

/// Custom scalar for HTML (Shopify returns HTML strings).
pub type HTML = String;

/// Custom scalar for Color (Shopify returns color hex strings).
pub type Color = String;

/// Custom scalar for JSON (Shopify returns JSON strings).
pub type JSON = serde_json::Value;

/// Custom scalar for UnsignedInt64 (Shopify returns large integers).
pub type UnsignedInt64 = String;

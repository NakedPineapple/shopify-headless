//! Cache types for Storefront API responses.

use crate::shopify::types::{Collection, CollectionConnection, Product, ProductConnection};

/// Cache key for products and collections.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum CacheKey {
    Product(String),
    Products { cursor: Option<String> },
    Collection(String),
    Collections { cursor: Option<String> },
}

/// Cached value types.
#[derive(Debug, Clone)]
pub enum CacheValue {
    Product(Box<Product>),
    Products(ProductConnection),
    Collection(Box<Collection>),
    Collections(CollectionConnection),
}

//! Dynamic tool selection for Claude chat.
//!
//! With 111+ Shopify tools available, we need intelligent selection rather than
//! passing all tools to Claude. This module implements a three-stage selection:
//!
//! 1. **Domain Classification (Haiku)** - Fast classifier determines 1-3 relevant domains
//! 2. **Embedding Retrieval (pgvector + `OpenAI`)** - Similar example queries filtered by domain
//! 3. **Tool Selection** - Map example queries to tools, return 5-10 unique tools
//!
//! ## Domains
//!
//! Tools are organized into domains:
//! - `orders` - Order management and queries
//! - `customers` - Customer management and queries
//! - `products` - Product catalog management
//! - `inventory` - Inventory tracking and adjustments
//! - `collections` - Collection management
//! - `discounts` - Discount and promotion management
//! - `gift_cards` - Gift card operations
//! - `fulfillment` - Fulfillment and shipping
//! - `finance` - Payouts, disputes, bank accounts
//! - `order_editing` - Order modification operations
//!
//! ## Learning
//!
//! The system learns from successful tool uses. When a query leads to successful
//! tool execution, it's added as a new example (with `is_learned = true`).

mod classifier;
mod embeddings;
mod error;
pub mod seeder;
mod selector;

pub use classifier::DomainClassifier;
pub use embeddings::EmbeddingClient;
pub use error::ToolSelectionError;
pub use seeder::{
    SeedResult, ToolExampleConfig, ToolExamplesConfig, seed_from_file, validate_config,
};
pub use selector::ToolSelector;

/// Available tool domains.
pub const DOMAINS: &[&str] = &[
    "analytics",
    "orders",
    "customers",
    "products",
    "inventory",
    "collections",
    "discounts",
    "gift_cards",
    "fulfillment",
    "finance",
    "order_editing",
];

/// Domain descriptions for the classifier.
pub const DOMAIN_DESCRIPTIONS: &[(&str, &str)] = &[
    (
        "analytics",
        "Business analytics: sales summaries, revenue trends, top products, customer insights, inventory reports",
    ),
    (
        "orders",
        "Order management: viewing, searching, updating, canceling, refunding orders",
    ),
    (
        "customers",
        "Customer management: profiles, addresses, marketing, segments, merging",
    ),
    (
        "products",
        "Product catalog: products, variants, pricing, media, publishing",
    ),
    (
        "inventory",
        "Inventory tracking: stock levels, adjustments, transfers between locations",
    ),
    (
        "collections",
        "Collection management: smart/manual collections, product organization",
    ),
    (
        "discounts",
        "Promotions: discount codes, automatic discounts, bulk operations",
    ),
    (
        "gift_cards",
        "Gift card operations: issuing, crediting, debiting, notifications",
    ),
    (
        "fulfillment",
        "Shipping: fulfillment orders, tracking, holds, returns",
    ),
    (
        "finance",
        "Financial: payouts, disputes, bank accounts, payment capture",
    ),
    (
        "order_editing",
        "Order modifications: adding/removing items, adjusting quantities, editing",
    ),
];

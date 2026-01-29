//! Domain types for Shopify Admin API.
//!
//! These types provide a clean, ergonomic API separate from the raw
//! `graphql_client` generated types.

pub mod analytics;
pub mod common;
pub mod customer;
pub mod discount;
pub mod gift_card;
pub mod inventory;
pub mod order;
pub mod order_edit;
pub mod payments;
pub mod product;
pub mod refund;

// Re-export all types for convenience
pub use analytics::*;
pub use common::*;
pub use customer::*;
pub use discount::*;
pub use gift_card::*;
pub use inventory::*;
pub use order::*;
pub use order_edit::*;
pub use payments::*;
pub use product::*;
pub use refund::*;

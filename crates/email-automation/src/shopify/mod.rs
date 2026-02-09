//! Shopify queries for email triage context.
//!
//! Provides order and product lookups needed by the response composer to
//! include real Shopify data in draft replies.

pub mod client;
pub mod fulfillments;
pub mod orders;
pub mod products;

pub use client::ShopifyClient;

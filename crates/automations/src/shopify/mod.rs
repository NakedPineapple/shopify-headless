//! Shopify queries for email triage context.
//!
//! Provides order and product lookups needed by the response composer to
//! include real Shopify data in draft replies.

pub mod checkouts;
pub mod client;
pub mod customers;
pub mod fulfillments;
pub mod inventory;
pub mod orders;
pub mod products;
pub mod subscriptions;

pub use client::ShopifyClient;

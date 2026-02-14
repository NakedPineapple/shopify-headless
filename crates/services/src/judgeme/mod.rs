//! Judge.me API client for product reviews.
//!
//! Provides methods to read, create, moderate, and reply to product reviews
//! via the Judge.me REST API. Used by both the storefront (display + submit)
//! and admin panel (moderation).

mod client;
mod error;
pub mod types;

pub use client::JudgemeClient;
pub use error::JudgemeError;

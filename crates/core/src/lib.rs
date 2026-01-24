//! Naked Pineapple Core - Shared types library.
//!
//! This crate provides common types used across all Naked Pineapple components:
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

//! Naked Pineapple Admin library.
//!
//! This crate provides the admin functionality as a library,
//! allowing it to be tested and reused.
//!
//! # Security
//!
//! This crate contains HIGH PRIVILEGE access:
//! - Shopify Admin API (full store management)
//! - Claude API (AI chat)
//! - Admin user management
//!
//! Only deploy on Tailscale-protected infrastructure with MDM verification.

#![cfg_attr(not(test), forbid(unsafe_code))]
// Allow dead code during incremental development - many features are not yet wired up
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod claude;
pub mod components;
pub mod config;
pub mod db;
pub mod error;
pub mod filters;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod services;
pub mod shiphero;
pub mod shopify;
pub mod slack;
pub mod state;
pub mod tool_selection;

//! Naked Pineapple Storefront library.
//!
//! This crate provides the storefront functionality as a library,
//! allowing it to be tested and reused.

#![cfg_attr(not(test), forbid(unsafe_code))]
// Allow dead code during incremental development - many features are not yet wired up
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod config;
pub mod content;
pub mod db;
pub mod error;
pub mod filters;
pub mod image_manifest;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod search;
pub mod services;
pub mod shopify;
pub mod state;

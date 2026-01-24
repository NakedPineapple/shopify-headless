//! Naked Pineapple Storefront library.
//!
//! This crate provides the storefront functionality as a library,
//! allowing it to be tested and reused.

#![cfg_attr(not(test), forbid(unsafe_code))]

pub mod config;
pub mod db;
pub mod error;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod services;
pub mod shopify;
pub mod state;

//! Shared customer support library for Pineapple Skin Co.
//!
//! Used by both storefront (customer-facing chat) and admin (inbox & ticketing).
//! All data lives in `np_storefront.storefront.support_*` tables.

#![cfg_attr(not(test), forbid(unsafe_code))]

pub mod db;
pub mod error;
pub mod models;
pub mod rag;
pub mod service;
pub mod tools;

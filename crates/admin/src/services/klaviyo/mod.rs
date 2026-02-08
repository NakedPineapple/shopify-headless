//! Klaviyo API client - re-exported from services crate.
//!
//! The client, types, and campaign operations live in `naked-pineapple-services`.
//! This module re-exports them for backwards compatibility within admin.

pub use naked_pineapple_services::klaviyo::*;

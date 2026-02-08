//! Claude API client - re-exported from services crate.
//!
//! The client implementation lives in `naked-pineapple-services`.
//! This module re-exports it for backwards compatibility within admin.

pub use naked_pineapple_services::claude::ClaudeClient;

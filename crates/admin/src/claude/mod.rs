//! Claude API integration for AI-powered admin chat.
//!
//! This module re-exports the Claude client from `naked-pineapple-services`
//! and provides admin-specific tool definitions and execution.
//!
//! # Architecture
//!
//! - `ClaudeClient` - HTTP client for the Claude Messages API (from services)
//! - `all_tools()` - All 127 tool definitions (document search + analytics + Shopify)
//! - `ToolExecutor` - Executes tools by calling the appropriate backend

mod client;
mod error;
pub mod tools;
pub mod types;

pub use client::ClaudeClient;
pub use error::ClaudeError;
pub use tools::{
    ToolExecutor, ToolResult, all_tools, filter_tools_by_names, get_tool_by_name, get_tool_domain,
    get_tools_by_domain, requires_confirmation,
};
pub use types::*;

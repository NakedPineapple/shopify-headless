//! Claude API integration for AI-powered admin chat.
//!
//! This module provides a client for interacting with the Anthropic Claude API,
//! including support for tool use to query Shopify data.
//!
//! # Architecture
//!
//! - `ClaudeClient` - HTTP client for the Claude Messages API
//! - `shopify_tools()` - Tool definitions for Shopify queries
//! - `ToolExecutor` - Executes tools by calling the Shopify Admin API
//!
//! # Example
//!
//! ```rust,ignore
//! use naked_pineapple_admin::claude::{ClaudeClient, shopify_tools, ToolExecutor};
//!
//! let client = ClaudeClient::new(&config.claude);
//! let tools = shopify_tools();
//!
//! // Send a chat message with tools available
//! let response = client.chat(
//!     vec![Message {
//!         role: "user".to_string(),
//!         content: MessageContent::Text("Show me recent orders".to_string()),
//!     }],
//!     Some(SYSTEM_PROMPT.to_string()),
//!     Some(tools),
//! ).await?;
//!
//! // If Claude wants to use a tool
//! if response.stop_reason == Some(StopReason::ToolUse) {
//!     let executor = ToolExecutor::new(&shopify_client);
//!     for block in &response.content {
//!         if let ContentBlock::ToolUse { id, name, input } = block {
//!             let result = executor.execute(name, input).await?;
//!             // Add tool result to conversation and continue...
//!         }
//!     }
//! }
//! ```

mod client;
mod error;
mod tools;
pub mod types;

pub use client::ClaudeClient;
pub use error::ClaudeError;
pub use tools::{ToolExecutor, shopify_tools};
pub use types::*;

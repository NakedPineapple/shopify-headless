//! Business logic services for admin.
//!
//! # Services
//!
//! - `chat` - Claude chat orchestration with tool execution

pub mod chat;

pub use chat::{ChatError, ChatService};

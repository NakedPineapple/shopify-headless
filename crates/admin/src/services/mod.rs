//! Business logic services for admin.
//!
//! # Services
//!
//! - `auth` - `WebAuthn` passkey-only authentication
//! - `chat` - Claude chat orchestration with tool execution

pub mod auth;
pub mod chat;

pub use auth::{AdminAuthError, AdminAuthService};
pub use chat::{ChatError, ChatService};

//! Business logic services for admin.
//!
//! # Services
//!
//! - `auth` - `WebAuthn` passkey-only authentication
//! - `chat` - Claude chat orchestration with tool execution
//! - `email` - Email delivery via SMTP

pub mod auth;
pub mod chat;
pub mod email;

pub use auth::{AdminAuthError, AdminAuthService};
pub use chat::{ChatError, ChatService};
pub use email::{EmailError, EmailService, generate_verification_code};

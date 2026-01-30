//! Business logic services for admin.
//!
//! # Services
//!
//! - `action_queue` - Pending action queue for Slack confirmations
//! - `auth` - `WebAuthn` passkey-only authentication
//! - `chat` - Claude chat orchestration with tool execution
//! - `email` - Email delivery via SMTP
//! - `klaviyo` - Klaviyo API client for newsletter campaigns

pub mod action_queue;
pub mod auth;
pub mod chat;
pub mod email;
pub mod klaviyo;

pub use action_queue::{ActionQueueService, EnqueueParams, EnqueueResult};
pub use auth::{AdminAuthError, AdminAuthService};
pub use chat::{ChatError, ChatService, ChatStreamEvent, stream_chat_message};
pub use email::{EmailError, EmailService, generate_verification_code};
pub use klaviyo::{KlaviyoClient, KlaviyoError};

//! Database operations for admin `PostgreSQL`.
//!
//! # Database: `np_admin` (SEPARATE from storefront)
//!
//! ## Tables
//!
//! - `admin_users` - Admin authentication (separate from storefront users)
//! - `admin_sessions` - Admin session storage
//! - `admin_credentials` - Admin `WebAuthn` passkeys
//! - `chat_sessions` - Claude AI chat sessions
//! - `chat_messages` - Chat message history (JSONB content)
//! - `shopify_tokens` - Encrypted OAuth tokens (if needed)
//! - `settings` - Application settings (JSONB)
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! pub mod admin_users;
//! pub mod chat;
//! pub mod credentials;
//! pub mod sessions;
//! pub mod settings;
//!
//! // Example: admin_users.rs
//! use sqlx::PgPool;
//! use crate::models::AdminUser;
//!
//! pub async fn get_admin_by_email(
//!     pool: &PgPool,
//!     email: &str,
//! ) -> Result<Option<AdminUser>, sqlx::Error> {
//!     sqlx::query_as!(
//!         AdminUser,
//!         r#"
//!         SELECT id, email, name, role, created_at, updated_at
//!         FROM admin_users
//!         WHERE email = $1
//!         "#,
//!         email
//!     )
//!     .fetch_optional(pool)
//!     .await
//! }
//!
//! // Example: chat.rs
//! pub async fn create_chat_session(
//!     pool: &PgPool,
//!     admin_user_id: i64,
//!     title: Option<&str>,
//! ) -> Result<ChatSession, sqlx::Error> {
//!     sqlx::query_as!(
//!         ChatSession,
//!         r#"
//!         INSERT INTO chat_sessions (admin_user_id, title)
//!         VALUES ($1, $2)
//!         RETURNING id, admin_user_id, title, created_at, updated_at
//!         "#,
//!         admin_user_id,
//!         title
//!     )
//!     .fetch_one(pool)
//!     .await
//! }
//!
//! pub async fn add_chat_message(
//!     pool: &PgPool,
//!     chat_session_id: i64,
//!     role: &str,
//!     content: serde_json::Value,
//! ) -> Result<ChatMessage, sqlx::Error> {
//!     sqlx::query_as!(
//!         ChatMessage,
//!         r#"
//!         INSERT INTO chat_messages (chat_session_id, role, content)
//!         VALUES ($1, $2, $3)
//!         RETURNING id, chat_session_id, role, content, created_at
//!         "#,
//!         chat_session_id,
//!         role,
//!         content
//!     )
//!     .fetch_one(pool)
//!     .await
//! }
//! ```
//!
//! # Migrations
//!
//! Migrations are stored in `crates/admin/migrations/` and run via:
//! ```bash
//! cargo run -p naked-pineapple-cli -- migrate admin
//! ```

// TODO: Implement database operations

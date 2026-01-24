//! Domain models for admin.
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use chrono::{DateTime, Utc};
//! use serde::{Deserialize, Serialize};
//! use naked_pineapple_core::{ChatRole, Email, EntityId};
//!
//! /// An admin user (separate from storefront users).
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct AdminUser {
//!     pub id: EntityId,
//!     pub email: Email,
//!     pub name: String,
//!     pub role: AdminRole,
//!     pub created_at: DateTime<Utc>,
//!     pub updated_at: DateTime<Utc>,
//! }
//!
//! /// Admin role for authorization.
//! #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
//! #[serde(rename_all = "snake_case")]
//! pub enum AdminRole {
//!     /// Full access to everything
//!     SuperAdmin,
//!     /// Can manage products, orders, customers
//!     Admin,
//!     /// Read-only access
//!     Viewer,
//! }
//!
//! /// A Claude chat session.
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct ChatSession {
//!     pub id: EntityId,
//!     pub admin_user_id: EntityId,
//!     pub title: Option<String>,
//!     pub created_at: DateTime<Utc>,
//!     pub updated_at: DateTime<Utc>,
//! }
//!
//! /// A message in a chat session.
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct ChatMessage {
//!     pub id: EntityId,
//!     pub chat_session_id: EntityId,
//!     pub role: ChatRole,
//!     pub content: serde_json::Value,  // Flexible for tool use
//!     pub created_at: DateTime<Utc>,
//! }
//!
//! /// Application settings stored in database.
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct Settings {
//!     pub id: EntityId,
//!     pub key: String,
//!     pub value: serde_json::Value,
//!     pub updated_at: DateTime<Utc>,
//! }
//! ```

// TODO: Implement models

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
//! # Migrations
//!
//! Migrations are stored in `crates/admin/migrations/` and run via:
//! ```bash
//! cargo run -p naked-pineapple-cli -- migrate admin
//! ```

pub mod admin_users;
pub mod chat;

use std::time::Duration;

use secrecy::ExposeSecret;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use thiserror::Error;

pub use admin_users::AdminUserRepository;
pub use chat::ChatRepository;

/// Errors that can occur during repository operations.
#[derive(Debug, Error)]
pub enum RepositoryError {
    /// Database error from sqlx.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Data in the database is corrupted or invalid.
    #[error("data corruption: {0}")]
    DataCorruption(String),

    /// Requested entity was not found.
    #[error("not found")]
    NotFound,

    /// Constraint violation (e.g., unique email).
    #[error("constraint violation: {0}")]
    Conflict(String),
}

/// Create a `PostgreSQL` connection pool with sensible defaults.
///
/// # Arguments
///
/// * `database_url` - `PostgreSQL` connection string (wrapped in `SecretString`)
///
/// # Errors
///
/// Returns `sqlx::Error` if the connection cannot be established.
pub async fn create_pool(database_url: &secrecy::SecretString) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(10))
        .connect(database_url.expose_secret())
        .await
}

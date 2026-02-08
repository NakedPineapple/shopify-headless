//! Database operations for storefront `PostgreSQL`.
//!
//! # Database: `np_storefront`
//!
//! Stores local data only (Shopify is source of truth for products/orders):
//!
//! ## Tables
//!
//! - `sessions` - Tower-sessions storage
//! - `shopify_cart_cache` - Persist Shopify cart IDs across sessions
//!
//! # Migrations
//!
//! Migrations are stored in `crates/storefront/migrations/` and run via:
//! ```bash
//! cargo run -p naked-pineapple-cli -- migrate storefront
//! ```

use std::time::Duration;

use secrecy::ExposeSecret;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use thiserror::Error;

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
    let max_conn: u32 = std::env::var("DATABASE_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let min_conn: u32 = std::env::var("DATABASE_MIN_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    PgPoolOptions::new()
        .max_connections(max_conn)
        .min_connections(min_conn)
        .acquire_timeout(Duration::from_secs(10))
        .connect(database_url.expose_secret())
        .await
}

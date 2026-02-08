//! Database operations for email automation.
//!
//! Connects to the `np_admin` database (shared with the admin binary).
//! New tables for email triage, automation logs, and outbound queues
//! are added via admin's migration directory.

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

    /// Requested entity was not found.
    #[error("not found")]
    NotFound,

    /// Constraint violation (e.g., unique message ID).
    #[error("constraint violation: {0}")]
    Conflict(String),
}

/// Create a `PostgreSQL` connection pool.
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

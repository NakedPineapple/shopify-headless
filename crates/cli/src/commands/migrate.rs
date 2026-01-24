//! Database migration commands.
//!
//! # Usage
//!
//! ```bash
//! # Run storefront migrations
//! np-cli migrate storefront
//!
//! # Run admin migrations
//! np-cli migrate admin
//!
//! # Run all migrations
//! np-cli migrate all
//! ```
//!
//! # Environment Variables
//!
//! - `STOREFRONT_DATABASE_URL` - `PostgreSQL` connection string for storefront
//! - `ADMIN_DATABASE_URL` - `PostgreSQL` connection string for admin
//!
//! # Migration Files
//!
//! Storefront migrations: `crates/storefront/migrations/`
//! Admin migrations: `crates/admin/migrations/`

use sqlx::PgPool;
use thiserror::Error;

/// Errors that can occur during migration.
#[derive(Debug, Error)]
pub enum MigrationError {
    /// Required environment variable is missing.
    #[error("Missing environment variable: {0}")]
    MissingEnvVar(&'static str),

    /// Database connection error.
    #[error("Database connection error: {0}")]
    Database(#[from] sqlx::Error),

    /// Migration execution error.
    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

/// Run storefront database migrations.
///
/// Connects to the database specified by `STOREFRONT_DATABASE_URL` and runs
/// all pending migrations from `crates/storefront/migrations/`.
pub async fn storefront() -> Result<(), MigrationError> {
    dotenvy::dotenv().ok();

    let database_url = std::env::var("STOREFRONT_DATABASE_URL")
        .map_err(|_| MigrationError::MissingEnvVar("STOREFRONT_DATABASE_URL"))?;

    tracing::info!("Connecting to storefront database...");
    let pool = PgPool::connect(&database_url).await?;

    tracing::info!("Running storefront migrations...");
    sqlx::migrate!("../storefront/migrations")
        .run(&pool)
        .await?;

    tracing::info!("Storefront migrations complete!");
    Ok(())
}

/// Run admin database migrations.
///
/// Connects to the database specified by `ADMIN_DATABASE_URL` and runs
/// all pending migrations from `crates/admin/migrations/`.
pub async fn admin() -> Result<(), MigrationError> {
    dotenvy::dotenv().ok();

    let database_url = std::env::var("ADMIN_DATABASE_URL")
        .map_err(|_| MigrationError::MissingEnvVar("ADMIN_DATABASE_URL"))?;

    tracing::info!("Connecting to admin database...");
    let pool = PgPool::connect(&database_url).await?;

    tracing::info!("Running admin migrations...");
    sqlx::migrate!("../admin/migrations")
        .run(&pool)
        .await?;

    tracing::info!("Admin migrations complete!");
    Ok(())
}

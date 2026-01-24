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
//! # Future Implementation
//!
//! ```rust,ignore
//! use sqlx::PgPool;
//!
//! pub async fn storefront() -> Result<(), MigrationError> {
//!     dotenvy::dotenv().ok();
//!
//!     let database_url = std::env::var("STOREFRONT_DATABASE_URL")
//!         .map_err(|_| MigrationError::MissingEnvVar("STOREFRONT_DATABASE_URL"))?;
//!
//!     tracing::info!("Connecting to storefront database...");
//!     let pool = PgPool::connect(&database_url).await?;
//!
//!     tracing::info!("Running storefront migrations...");
//!     sqlx::migrate!("../storefront/migrations")
//!         .run(&pool)
//!         .await?;
//!
//!     tracing::info!("Storefront migrations complete!");
//!     Ok(())
//! }
//!
//! pub async fn admin() -> Result<(), MigrationError> {
//!     dotenvy::dotenv().ok();
//!
//!     let database_url = std::env::var("ADMIN_DATABASE_URL")
//!         .map_err(|_| MigrationError::MissingEnvVar("ADMIN_DATABASE_URL"))?;
//!
//!     tracing::info!("Connecting to admin database...");
//!     let pool = PgPool::connect(&database_url).await?;
//!
//!     tracing::info!("Running admin migrations...");
//!     sqlx::migrate!("../admin/migrations")
//!         .run(&pool)
//!         .await?;
//!
//!     tracing::info!("Admin migrations complete!");
//!     Ok(())
//! }
//!
//! #[derive(Debug, thiserror::Error)]
//! pub enum MigrationError {
//!     #[error("Missing environment variable: {0}")]
//!     MissingEnvVar(&'static str),
//!
//!     #[error("Database error: {0}")]
//!     Database(#[from] sqlx::Error),
//!
//!     #[error("Migration error: {0}")]
//!     Migration(#[from] sqlx::migrate::MigrateError),
//! }
//! ```
//!
//! # Migration Files
//!
//! Storefront migrations: `crates/storefront/migrations/`
//! Admin migrations: `crates/admin/migrations/`
//!
//! Example migration structure:
//! ```
//! migrations/
//! ├── 20260124000001_create_users.sql
//! ├── 20260124000002_create_sessions.sql
//! ├── 20260124000003_create_credentials.sql
//! └── ...
//! ```

/// Run storefront database migrations.
pub fn storefront() {
    // TODO: Load STOREFRONT_DATABASE_URL from env
    // TODO: Connect to database
    // TODO: Run migrations from crates/storefront/migrations/

    #[allow(clippy::print_stdout)]
    {
        println!("TODO: Run storefront migrations");
        println!("  1. Load STOREFRONT_DATABASE_URL from environment");
        println!("  2. Connect to PostgreSQL database");
        println!("  3. Run migrations from crates/storefront/migrations/");
    }
}

/// Run admin database migrations.
pub fn admin() {
    // TODO: Load ADMIN_DATABASE_URL from env
    // TODO: Connect to database
    // TODO: Run migrations from crates/admin/migrations/

    #[allow(clippy::print_stdout)]
    {
        println!("TODO: Run admin migrations");
        println!("  1. Load ADMIN_DATABASE_URL from environment");
        println!("  2. Connect to PostgreSQL database");
        println!("  3. Run migrations from crates/admin/migrations/");
    }
}

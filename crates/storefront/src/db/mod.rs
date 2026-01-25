//! Database operations for storefront `PostgreSQL`.
//!
//! # Database: `np_storefront`
//!
//! Stores local data only (Shopify is source of truth for products/orders):
//!
//! ## Tables
//!
//! - `users` - Site authentication (separate from Shopify customers)
//! - `sessions` - Tower-sessions storage
//! - `user_credentials` - `WebAuthn` passkeys
//! - `password_reset_tokens`
//! - `email_verification_codes`
//! - `addresses` - User shipping/billing addresses
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

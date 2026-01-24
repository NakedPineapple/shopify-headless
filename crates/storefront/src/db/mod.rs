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
//! # Future Implementation
//!
//! ```rust,ignore
//! pub mod users;
//! pub mod sessions;
//! pub mod credentials;
//! pub mod addresses;
//! pub mod cart_cache;
//!
//! // Example: users.rs
//! use sqlx::PgPool;
//! use crate::models::User;
//!
//! pub async fn get_user_by_email(
//!     pool: &PgPool,
//!     email: &str,
//! ) -> Result<Option<User>, sqlx::Error> {
//!     sqlx::query_as!(
//!         User,
//!         r#"
//!         SELECT id, email, password_hash, email_verified, created_at, updated_at
//!         FROM users
//!         WHERE email = $1
//!         "#,
//!         email
//!     )
//!     .fetch_optional(pool)
//!     .await
//! }
//!
//! pub async fn create_user(
//!     pool: &PgPool,
//!     email: &str,
//!     password_hash: &str,
//! ) -> Result<User, sqlx::Error> {
//!     sqlx::query_as!(
//!         User,
//!         r#"
//!         INSERT INTO users (email, password_hash)
//!         VALUES ($1, $2)
//!         RETURNING id, email, password_hash, email_verified, created_at, updated_at
//!         "#,
//!         email,
//!         password_hash
//!     )
//!     .fetch_one(pool)
//!     .await
//! }
//! ```
//!
//! # Migrations
//!
//! Migrations are stored in `crates/storefront/migrations/` and run via:
//! ```bash
//! cargo run -p naked-pineapple-cli -- migrate storefront
//! ```

// TODO: Implement database operations

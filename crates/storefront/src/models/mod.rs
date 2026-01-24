//! Domain models for storefront.
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use chrono::{DateTime, Utc};
//! use serde::{Deserialize, Serialize};
//! use naked_pineapple_core::{Email, EntityId};
//!
//! /// A storefront user (separate from Shopify customers).
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct User {
//!     pub id: EntityId,
//!     pub email: Email,
//!     pub password_hash: Option<String>,
//!     pub email_verified: bool,
//!     pub created_at: DateTime<Utc>,
//!     pub updated_at: DateTime<Utc>,
//! }
//!
//! /// A WebAuthn credential for passwordless login.
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct UserCredential {
//!     pub id: EntityId,
//!     pub user_id: EntityId,
//!     pub credential_id: Vec<u8>,
//!     pub public_key: Vec<u8>,
//!     pub counter: u32,
//!     pub name: String,
//!     pub created_at: DateTime<Utc>,
//! }
//!
//! /// A user's shipping or billing address.
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct Address {
//!     pub id: EntityId,
//!     pub user_id: EntityId,
//!     pub first_name: String,
//!     pub last_name: String,
//!     pub address1: String,
//!     pub address2: Option<String>,
//!     pub city: String,
//!     pub province: String,
//!     pub province_code: String,
//!     pub country: String,
//!     pub country_code: String,
//!     pub zip: String,
//!     pub phone: Option<String>,
//!     pub is_default: bool,
//!     pub created_at: DateTime<Utc>,
//!     pub updated_at: DateTime<Utc>,
//! }
//!
//! /// Cache entry for Shopify cart IDs.
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct CartCache {
//!     pub session_id: String,
//!     pub shopify_cart_id: String,
//!     pub created_at: DateTime<Utc>,
//!     pub updated_at: DateTime<Utc>,
//! }
//! ```

// TODO: Implement models

//! User domain types.
//!
//! These types represent validated domain objects separate from database row types.

use chrono::{DateTime, Utc};
use webauthn_rs::prelude::Passkey;

use naked_pineapple_core::{CredentialId, Email, UserId};

/// A storefront user (domain type).
///
/// This represents a local user record. With Shopify Storefront API authentication,
/// most user data is stored in Shopify, but we keep this for legacy support.
#[derive(Debug, Clone)]
pub struct User {
    /// Unique user ID.
    pub id: UserId,
    /// User's email address.
    pub email: Email,
    /// Whether the email has been verified.
    pub email_verified: bool,
    /// When the user was created.
    pub created_at: DateTime<Utc>,
    /// When the user was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A `WebAuthn` credential (domain type).
///
/// Users can have multiple passkeys for different devices.
/// Credentials are linked to Shopify customers via `shopify_customer_id`.
#[derive(Debug, Clone)]
pub struct UserCredential {
    /// Database ID of this credential.
    pub id: CredentialId,
    /// Shopify customer ID (e.g., `gid://shopify/Customer/123`).
    /// This links the credential to a Shopify customer.
    pub shopify_customer_id: String,
    /// Customer's email address (for passkey-by-email lookup).
    /// Stored at registration time to enable passwordless login.
    pub email: Option<Email>,
    /// Legacy local user ID (for backwards compatibility during migration).
    /// Will be None for new credentials created after migration.
    pub user_id: Option<UserId>,
    /// `WebAuthn` credential ID (from the authenticator).
    pub webauthn_id: Vec<u8>,
    /// The full passkey data including public key.
    pub passkey: Passkey,
    /// User-assigned name for this credential (e.g., "MacBook", "iPhone").
    pub name: String,
    /// When this credential was registered.
    pub created_at: DateTime<Utc>,
}

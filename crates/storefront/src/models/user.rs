//! User domain types.
//!
//! These types represent validated domain objects separate from database row types.

use chrono::{DateTime, Utc};
use webauthn_rs::prelude::Passkey;

use naked_pineapple_core::{CredentialId, Email, UserId};

/// A storefront user (domain type).
///
/// Separate from Shopify customers - this is for local authentication only.
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
#[derive(Debug, Clone)]
pub struct UserCredential {
    /// Database ID of this credential.
    pub id: CredentialId,
    /// User who owns this credential.
    pub user_id: UserId,
    /// `WebAuthn` credential ID (from the authenticator).
    pub webauthn_id: Vec<u8>,
    /// The full passkey data including public key.
    pub passkey: Passkey,
    /// User-assigned name for this credential (e.g., "MacBook", "iPhone").
    pub name: String,
    /// When this credential was registered.
    pub created_at: DateTime<Utc>,
}

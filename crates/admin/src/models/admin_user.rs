//! Admin user domain types.
//!
//! These types represent validated domain objects for admin authentication.

use chrono::{DateTime, Utc};
use uuid::Uuid;
use webauthn_rs::prelude::Passkey;

use naked_pineapple_core::{AdminCredentialId, AdminUserId, Email};

// Re-export AdminRole from core for convenience
pub use naked_pineapple_core::AdminRole;

/// An admin user (domain type).
#[derive(Debug, Clone)]
pub struct AdminUser {
    /// Unique admin user ID.
    pub id: AdminUserId,
    /// Admin's email address.
    pub email: Email,
    /// Admin's display name.
    pub name: String,
    /// Admin's role/permission level.
    pub role: AdminRole,
    /// `WebAuthn` user ID for discoverable credentials (passkey login without email).
    /// This UUID is stored in the passkey and returned during authentication.
    pub webauthn_user_id: Uuid,
    /// Slack user ID for receiving DM notifications (e.g., "U0123456789").
    pub slack_user_id: Option<String>,
    /// When the admin was created.
    pub created_at: DateTime<Utc>,
    /// When the admin was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A `WebAuthn` credential for admin authentication (domain type).
///
/// Admin users can have multiple passkeys for different devices.
#[derive(Debug, Clone)]
pub struct AdminCredential {
    /// Database ID of this credential.
    pub id: AdminCredentialId,
    /// Admin user who owns this credential.
    pub admin_user_id: AdminUserId,
    /// `WebAuthn` credential ID (from the authenticator).
    pub webauthn_id: Vec<u8>,
    /// The full passkey data including public key.
    pub passkey: Passkey,
    /// User-assigned name for this credential (e.g., "MacBook", "iPhone").
    pub name: String,
    /// When this credential was registered.
    pub created_at: DateTime<Utc>,
}

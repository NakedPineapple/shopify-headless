//! Session-related types for admin authentication.
//!
//! Types stored in the session for authentication state.

use serde::{Deserialize, Serialize};

use naked_pineapple_core::{AdminUserId, Email};

use super::admin_user::AdminRole;

/// Session-stored admin identity.
///
/// Minimal data stored in the session to identify the logged-in admin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentAdmin {
    /// Admin's database ID.
    pub id: AdminUserId,
    /// Admin's email address.
    pub email: Email,
    /// Admin's display name.
    pub name: String,
    /// Admin's role/permission level.
    pub role: AdminRole,
}

/// Session keys for admin authentication data.
pub mod keys {
    /// Key for storing the current logged-in admin.
    pub const CURRENT_ADMIN: &str = "current_admin";

    /// Key for `WebAuthn` registration challenge state.
    pub const WEBAUTHN_REG: &str = "webauthn_reg";

    /// Key for `WebAuthn` authentication challenge state.
    pub const WEBAUTHN_AUTH: &str = "webauthn_auth";
}

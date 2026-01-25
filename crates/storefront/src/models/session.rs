//! Session-related types.
//!
//! Types stored in the session for authentication state.

use serde::{Deserialize, Serialize};

use naked_pineapple_core::{Email, UserId};

/// Session-stored user identity.
///
/// Minimal data stored in the session to identify the logged-in user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentUser {
    /// User's database ID.
    pub id: UserId,
    /// User's email address.
    pub email: Email,
}

/// Session keys for authentication data.
pub mod keys {
    /// Key for storing the current logged-in user.
    pub const CURRENT_USER: &str = "current_user";

    /// Key for `WebAuthn` registration challenge state.
    pub const WEBAUTHN_REG: &str = "webauthn_reg";

    /// Key for `WebAuthn` authentication challenge state.
    pub const WEBAUTHN_AUTH: &str = "webauthn_auth";

    /// Key for storing the Shopify cart ID.
    pub const CART_ID: &str = "cart_id";

    /// Key for Shopify OAuth state (CSRF protection).
    pub const SHOPIFY_OAUTH_STATE: &str = "shopify_oauth_state";

    /// Key for Shopify OAuth nonce (`OpenID` Connect replay protection).
    pub const SHOPIFY_OAUTH_NONCE: &str = "shopify_oauth_nonce";

    /// Key for Shopify customer access token.
    pub const SHOPIFY_CUSTOMER_TOKEN: &str = "shopify_customer_token";
}

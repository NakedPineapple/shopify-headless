//! Session-related types.
//!
//! Types stored in the session for authentication state.

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use naked_pineapple_core::Email;

/// Session-stored customer identity for Storefront API authentication.
///
/// This represents a customer authenticated via the Shopify Storefront API
/// (email/password login), as opposed to OAuth via Customer Account API.
#[derive(Clone, Serialize, Deserialize)]
pub struct CurrentCustomer {
    /// Shopify customer ID (e.g., `gid://shopify/Customer/123`)
    pub shopify_customer_id: String,
    /// Customer's email address
    pub email: String,
    /// Customer's first name (optional)
    pub first_name: Option<String>,
    /// Customer's last name (optional)
    pub last_name: Option<String>,
    /// Storefront API access token (secret to prevent accidental logging)
    #[serde(with = "secret_string")]
    access_token: SecretString,
    /// When the access token expires (ISO 8601 format)
    pub access_token_expires_at: String,
}

impl std::fmt::Debug for CurrentCustomer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CurrentCustomer")
            .field("shopify_customer_id", &self.shopify_customer_id)
            .field("email", &self.email)
            .field("first_name", &self.first_name)
            .field("last_name", &self.last_name)
            .field("access_token", &"[REDACTED]")
            .field("access_token_expires_at", &self.access_token_expires_at)
            .finish()
    }
}

impl CurrentCustomer {
    /// Create a new `CurrentCustomer`.
    #[must_use]
    pub const fn new(
        shopify_customer_id: String,
        email: String,
        first_name: Option<String>,
        last_name: Option<String>,
        access_token: SecretString,
        access_token_expires_at: String,
    ) -> Self {
        Self {
            shopify_customer_id,
            email,
            first_name,
            last_name,
            access_token,
            access_token_expires_at,
        }
    }

    /// Get the access token for making authenticated API calls.
    #[must_use]
    pub const fn access_token(&self) -> &SecretString {
        &self.access_token
    }

    /// Get the email as an Email type.
    ///
    /// # Errors
    ///
    /// Returns an error if the email is invalid (should not happen as Shopify validates).
    pub fn email_parsed(&self) -> Result<Email, naked_pineapple_core::EmailError> {
        Email::parse(&self.email)
    }

    /// Check if the access token has expired.
    ///
    /// Returns `true` if the token is expired or cannot be parsed.
    #[must_use]
    pub fn is_token_expired(&self) -> bool {
        chrono::DateTime::parse_from_rfc3339(&self.access_token_expires_at)
            .map(|expires_at| expires_at < chrono::Utc::now())
            .unwrap_or(true)
    }
}

/// Serde helper for serializing/deserializing `SecretString`.
mod secret_string {
    use secrecy::{ExposeSecret, SecretString};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(secret: &SecretString, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(secret.expose_secret())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SecretString, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(SecretString::from(s))
    }
}

/// Session keys for authentication data.
pub mod keys {
    /// Key for storing the current Shopify customer (Storefront API auth).
    pub const CURRENT_CUSTOMER: &str = "current_customer";

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

    /// Key for Shopify customer access token (Customer Account API OAuth).
    pub const SHOPIFY_CUSTOMER_TOKEN: &str = "shopify_customer_token";
}

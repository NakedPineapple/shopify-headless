//! `WebAuthn` credential types.
//!
//! Type-safe wrappers for `WebAuthn` credential data.

use serde::{Deserialize, Serialize};

/// `WebAuthn` credential identifier (from authenticator).
///
/// This is the raw credential ID bytes returned by the authenticator
/// during registration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WebAuthnCredentialId(Vec<u8>);

impl WebAuthnCredentialId {
    /// Create a new `WebAuthn` credential ID.
    #[must_use]
    pub const fn new(id: Vec<u8>) -> Self {
        Self(id)
    }

    /// Get the credential ID as a byte slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Convert into the inner bytes.
    #[must_use]
    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl AsRef<[u8]> for WebAuthnCredentialId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for WebAuthnCredentialId {
    fn from(id: Vec<u8>) -> Self {
        Self(id)
    }
}

impl From<WebAuthnCredentialId> for Vec<u8> {
    fn from(id: WebAuthnCredentialId) -> Self {
        id.0
    }
}

/// Serialized passkey for database storage.
///
/// This wraps the JSON-serialized `Passkey` from `webauthn-rs` for
/// type-safe storage in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StoredPasskey(Vec<u8>);

impl StoredPasskey {
    /// Create a new stored passkey from serialized bytes.
    #[must_use]
    pub const fn new(data: Vec<u8>) -> Self {
        Self(data)
    }

    /// Get the passkey data as a byte slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Convert into the inner bytes.
    #[must_use]
    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl AsRef<[u8]> for StoredPasskey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for StoredPasskey {
    fn from(data: Vec<u8>) -> Self {
        Self(data)
    }
}

impl From<StoredPasskey> for Vec<u8> {
    fn from(passkey: StoredPasskey) -> Self {
        passkey.0
    }
}

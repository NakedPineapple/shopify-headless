//! Admin authentication error types.

use thiserror::Error;

use crate::db::RepositoryError;

/// Errors that can occur during admin authentication operations.
#[derive(Debug, Error)]
pub enum AdminAuthError {
    /// Invalid email format.
    #[error("invalid email: {0}")]
    InvalidEmail(#[from] naked_pineapple_core::EmailError),

    /// Admin user not found.
    #[error("admin user not found")]
    UserNotFound,

    /// Admin user already exists.
    #[error("admin user already exists")]
    UserAlreadyExists,

    /// `WebAuthn` error.
    #[error("webauthn error: {0}")]
    WebAuthn(#[from] webauthn_rs::prelude::WebauthnError),

    /// No credentials registered for admin user.
    #[error("no passkeys registered for this account")]
    NoCredentials,

    /// Credential not found.
    #[error("credential not found")]
    CredentialNotFound,

    /// Cannot delete the last credential.
    #[error("cannot delete your only passkey")]
    LastCredential,

    /// Invalid user handle from passkey authentication.
    #[error("invalid user handle in passkey")]
    InvalidUserHandle,

    /// Session state missing or invalid.
    #[error("invalid session state")]
    InvalidSessionState,

    /// Repository/database error.
    #[error("database error: {0}")]
    Repository(#[from] RepositoryError),
}

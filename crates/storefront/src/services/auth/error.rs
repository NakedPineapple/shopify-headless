//! Authentication error types.

use thiserror::Error;

use crate::db::RepositoryError;

/// Errors that can occur during authentication operations.
#[derive(Debug, Error)]
pub enum AuthError {
    /// Invalid email format.
    #[error("invalid email: {0}")]
    InvalidEmail(#[from] naked_pineapple_core::EmailError),

    /// Invalid credentials (wrong password or user not found).
    #[error("invalid credentials")]
    InvalidCredentials,

    /// User not found.
    #[error("user not found")]
    UserNotFound,

    /// User already exists.
    #[error("user already exists")]
    UserAlreadyExists,

    /// Password too weak or invalid.
    #[error("password validation failed: {0}")]
    WeakPassword(String),

    /// `WebAuthn` error.
    #[error("webauthn error: {0}")]
    WebAuthn(#[from] webauthn_rs::prelude::WebauthnError),

    /// No credentials registered for user.
    #[error("no passkeys registered for this account")]
    NoCredentials,

    /// Credential not found.
    #[error("credential not found")]
    CredentialNotFound,

    /// Session state missing or invalid.
    #[error("invalid session state")]
    InvalidSessionState,

    /// Repository/database error.
    #[error("database error: {0}")]
    Repository(#[from] RepositoryError),

    /// Password hashing error.
    #[error("password hashing error")]
    PasswordHash,
}

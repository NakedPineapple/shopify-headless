//! Cryptographic utilities for Naked Pineapple.
//!
//! Provides Argon2id password hashing and verification, shared by the admin
//! authentication service and the CLI password management command.

#![cfg_attr(not(test), forbid(unsafe_code))]

use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

/// Minimum password length.
const MIN_PASSWORD_LENGTH: usize = 12;

/// Errors that can occur during password operations.
#[derive(Debug, Error)]
pub enum PasswordError {
    /// Password does not meet minimum length requirement.
    #[error("password must be at least {MIN_PASSWORD_LENGTH} characters")]
    TooShort,

    /// Password hashing or verification failed.
    #[error("password processing error: {0}")]
    Hash(String),
}

/// Hash a password using Argon2id with a random salt.
///
/// Enforces a minimum length of 12 characters.
///
/// # Errors
///
/// Returns `PasswordError::TooShort` if the password is under 12 characters.
/// Returns `PasswordError::Hash` if Argon2 hashing fails.
pub fn hash_password(password: &SecretString) -> Result<String, PasswordError> {
    if password.expose_secret().len() < MIN_PASSWORD_LENGTH {
        return Err(PasswordError::TooShort);
    }

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.expose_secret().as_bytes(), &salt)
        .map_err(|e| PasswordError::Hash(e.to_string()))?;
    Ok(hash.to_string())
}

/// Verify a password against a stored Argon2id hash.
///
/// # Errors
///
/// Returns `PasswordError::Hash` if the stored hash is malformed.
pub fn verify_password(password: &SecretString, hash: &str) -> Result<bool, PasswordError> {
    let parsed = PasswordHash::new(hash).map_err(|e| PasswordError::Hash(e.to_string()))?;
    Ok(Argon2::default()
        .verify_password(password.expose_secret().as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let password = SecretString::from("a_valid_password_123");
        let hash = hash_password(&password).expect("hashing should succeed");
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password(&password, &hash).expect("verify should succeed"));
    }

    #[test]
    fn test_wrong_password() {
        let password = SecretString::from("a_valid_password_123");
        let hash = hash_password(&password).expect("hashing should succeed");
        let wrong = SecretString::from("wrong_password_456");
        assert!(!verify_password(&wrong, &hash).expect("verify should succeed"));
    }

    #[test]
    fn test_too_short() {
        let password = SecretString::from("short");
        let err = hash_password(&password).expect_err("should reject short password");
        assert!(matches!(err, PasswordError::TooShort));
    }

    #[test]
    fn test_invalid_hash() {
        let password = SecretString::from("a_valid_password_123");
        let err =
            verify_password(&password, "not-a-valid-hash").expect_err("should reject invalid hash");
        assert!(matches!(err, PasswordError::Hash(_)));
    }
}

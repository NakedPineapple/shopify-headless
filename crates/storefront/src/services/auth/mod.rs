//! Authentication service.
//!
//! Provides password and `WebAuthn` passkey authentication.

mod error;

pub use error::AuthError;

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use sqlx::PgPool;
use webauthn_rs::prelude::*;

use naked_pineapple_core::{Email, UserId};

use crate::db::RepositoryError;
use crate::db::users::UserRepository;
use crate::models::user::{User, UserCredential};

/// Minimum password length.
const MIN_PASSWORD_LENGTH: usize = 8;

/// Authentication service.
///
/// Handles user registration, login, and `WebAuthn` passkey management.
pub struct AuthService<'a> {
    users: UserRepository<'a>,
    webauthn: &'a Webauthn,
}

impl<'a> AuthService<'a> {
    /// Create a new authentication service.
    #[must_use]
    pub const fn new(pool: &'a PgPool, webauthn: &'a Webauthn) -> Self {
        Self {
            users: UserRepository::new(pool),
            webauthn,
        }
    }

    // =========================================================================
    // Password Authentication
    // =========================================================================

    /// Register a new user with email and password.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::InvalidEmail` if the email format is invalid.
    /// Returns `AuthError::WeakPassword` if the password doesn't meet requirements.
    /// Returns `AuthError::UserAlreadyExists` if the email is already registered.
    pub async fn register_with_password(
        &self,
        email: &str,
        password: &str,
    ) -> Result<User, AuthError> {
        // Validate email
        let email = Email::parse(email)?;

        // Validate password
        validate_password(password)?;

        // Hash password
        let password_hash = hash_password(password)?;

        // Create user
        let user = self
            .users
            .create_with_password(&email, &password_hash)
            .await
            .map_err(|e| match e {
                RepositoryError::Conflict(_) => AuthError::UserAlreadyExists,
                other => AuthError::Repository(other),
            })?;

        Ok(user)
    }

    /// Login with email and password.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::InvalidCredentials` if the email/password is wrong.
    pub async fn login_with_password(
        &self,
        email: &str,
        password: &str,
    ) -> Result<User, AuthError> {
        // Validate email format
        let email = Email::parse(email)?;

        // Get user with password hash
        let (user, password_hash) = self
            .users
            .get_password_hash(&email)
            .await?
            .ok_or(AuthError::InvalidCredentials)?;

        // Verify password
        verify_password(password, &password_hash)?;

        Ok(user)
    }

    // =========================================================================
    // WebAuthn Registration
    // =========================================================================

    /// Start passkey registration for an existing user.
    ///
    /// Returns the challenge to send to the client and the registration state
    /// to store in the session.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::WebAuthn` if the challenge cannot be generated.
    pub fn start_passkey_registration(
        &self,
        user: &User,
        existing_credentials: &[UserCredential],
    ) -> Result<(CreationChallengeResponse, PasskeyRegistration), AuthError> {
        // Collect existing credential IDs to exclude
        let exclude_credentials: Vec<CredentialID> = existing_credentials
            .iter()
            .map(|c| CredentialID::from(c.webauthn_id.clone()))
            .collect();

        // Create challenge
        let (challenge, reg_state) = self.webauthn.start_passkey_registration(
            Uuid::new_v4(),
            user.email.as_str(),
            user.email.as_str(),
            Some(exclude_credentials),
        )?;

        Ok((challenge, reg_state))
    }

    /// Finish passkey registration.
    ///
    /// Validates the client's response and returns the passkey to store.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::WebAuthn` if validation fails.
    pub fn finish_passkey_registration(
        &self,
        state: &PasskeyRegistration,
        response: &RegisterPublicKeyCredential,
    ) -> Result<Passkey, AuthError> {
        let passkey = self.webauthn.finish_passkey_registration(response, state)?;

        Ok(passkey)
    }

    /// Save a registered credential to the database.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::Repository` if the database operation fails.
    pub async fn save_credential(
        &self,
        user_id: UserId,
        passkey: &Passkey,
        name: &str,
    ) -> Result<UserCredential, AuthError> {
        let credential = self.users.create_credential(user_id, passkey, name).await?;
        Ok(credential)
    }

    // =========================================================================
    // WebAuthn Authentication
    // =========================================================================

    /// Start passkey authentication for a user.
    ///
    /// Returns the challenge to send to the client and the authentication state
    /// to store in the session.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::UserNotFound` if the user doesn't exist.
    /// Returns `AuthError::NoCredentials` if the user has no registered passkeys.
    /// Returns `AuthError::WebAuthn` if the challenge cannot be generated.
    pub async fn start_passkey_authentication(
        &self,
        email: &str,
    ) -> Result<(RequestChallengeResponse, PasskeyAuthentication, UserId), AuthError> {
        // Validate and find user
        let email = Email::parse(email)?;
        let user = self
            .users
            .get_by_email(&email)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        // Get credentials
        let credentials = self.users.get_credentials(user.id).await?;
        if credentials.is_empty() {
            return Err(AuthError::NoCredentials);
        }

        // Get passkeys for WebAuthn
        let passkeys: Vec<Passkey> = credentials.iter().map(|c| c.passkey.clone()).collect();

        // Create challenge
        let (challenge, auth_state) = self.webauthn.start_passkey_authentication(&passkeys)?;

        Ok((challenge, auth_state, user.id))
    }

    /// Finish passkey authentication.
    ///
    /// Validates the client's response and returns the authenticated user.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::WebAuthn` if validation fails.
    /// Returns `AuthError::CredentialNotFound` if the credential isn't found.
    pub async fn finish_passkey_authentication(
        &self,
        state: &PasskeyAuthentication,
        response: &PublicKeyCredential,
        user_id: UserId,
    ) -> Result<User, AuthError> {
        // Verify the authentication
        let auth_result = self
            .webauthn
            .finish_passkey_authentication(response, state)?;

        // Update credential if needed
        if auth_result.needs_update() {
            // Find the credential that was used
            let cred_id = auth_result.cred_id();
            if let Some(mut credential) = self
                .users
                .get_credential_by_webauthn_id(cred_id.as_ref())
                .await?
            {
                // Update the passkey with new data
                credential.passkey.update_credential(&auth_result);
                self.users
                    .update_credential(cred_id.as_ref(), &credential.passkey)
                    .await?;
            }
        }

        // Get the user
        let user = self
            .users
            .get_by_id(user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?;

        Ok(user)
    }

    // =========================================================================
    // Credential Management
    // =========================================================================

    /// Get all credentials for a user.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::Repository` if the database operation fails.
    pub async fn get_credentials(&self, user_id: UserId) -> Result<Vec<UserCredential>, AuthError> {
        let credentials = self.users.get_credentials(user_id).await?;
        Ok(credentials)
    }

    /// Delete a credential.
    ///
    /// # Returns
    ///
    /// Returns `true` if the credential was deleted, `false` if it didn't exist.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::Repository` if the database operation fails.
    pub async fn delete_credential(
        &self,
        user_id: UserId,
        credential_id: naked_pineapple_core::CredentialId,
    ) -> Result<bool, AuthError> {
        let deleted = self.users.delete_credential(user_id, credential_id).await?;
        Ok(deleted)
    }

    /// Get a user by ID.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::UserNotFound` if the user doesn't exist.
    pub async fn get_user(&self, user_id: UserId) -> Result<User, AuthError> {
        self.users
            .get_by_id(user_id)
            .await?
            .ok_or(AuthError::UserNotFound)
    }

    // =========================================================================
    // Shopify Customer WebAuthn Methods
    // =========================================================================

    /// Get all credentials for a Shopify customer.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::Repository` if the database operation fails.
    pub async fn get_credentials_by_shopify_customer_id(
        &self,
        shopify_customer_id: &str,
    ) -> Result<Vec<UserCredential>, AuthError> {
        let credentials = self
            .users
            .get_credentials_by_shopify_customer_id(shopify_customer_id)
            .await?;
        Ok(credentials)
    }

    /// Start passkey registration for a Shopify customer.
    ///
    /// Returns the challenge to send to the client and the registration state
    /// to store in the session.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::WebAuthn` if the challenge cannot be generated.
    pub fn start_passkey_registration_for_shopify_customer(
        &self,
        shopify_customer_id: &str,
        email: &str,
        existing_credentials: &[UserCredential],
    ) -> Result<(CreationChallengeResponse, PasskeyRegistration), AuthError> {
        // Collect existing credential IDs to exclude
        let exclude_credentials: Vec<CredentialID> = existing_credentials
            .iter()
            .map(|c| CredentialID::from(c.webauthn_id.clone()))
            .collect();

        // Use Shopify customer ID as the user UUID (hash it to get a consistent UUID)
        let user_uuid = uuid_from_shopify_customer_id(shopify_customer_id);

        // Create challenge
        let (challenge, reg_state) = self.webauthn.start_passkey_registration(
            user_uuid,
            email,
            email,
            Some(exclude_credentials),
        )?;

        Ok((challenge, reg_state))
    }

    /// Save a registered credential for a Shopify customer.
    ///
    /// The email is stored to enable passkey-by-email lookup for passwordless authentication.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::Repository` if the database operation fails.
    pub async fn save_credential_for_shopify_customer(
        &self,
        shopify_customer_id: &str,
        email: &Email,
        passkey: &Passkey,
        name: &str,
    ) -> Result<UserCredential, AuthError> {
        let credential = self
            .users
            .create_credential_for_shopify_customer(shopify_customer_id, email, passkey, name)
            .await?;
        Ok(credential)
    }

    /// Start passkey authentication for a Shopify customer.
    ///
    /// Looks up credentials by email address stored during passkey registration.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::InvalidEmail` if the email format is invalid.
    /// Returns `AuthError::NoCredentials` if no passkeys are registered for this email.
    /// Returns `AuthError::WebAuthn` if the challenge cannot be generated.
    pub async fn start_passkey_authentication_for_shopify_customer(
        &self,
        email: &str,
    ) -> Result<(RequestChallengeResponse, PasskeyAuthentication, String), AuthError> {
        // Parse and validate email
        let email = Email::parse(email)?;

        // Look up credentials by email
        let credentials = self.users.get_credentials_by_email(&email).await?;

        // Get the Shopify customer ID from the first credential
        // (all credentials for the same email should have the same customer ID)
        let Some(first_credential) = credentials.first() else {
            return Err(AuthError::NoCredentials);
        };
        let shopify_customer_id = first_credential.shopify_customer_id.clone();

        // Get passkeys for WebAuthn
        let passkeys: Vec<Passkey> = credentials.iter().map(|c| c.passkey.clone()).collect();

        // Create challenge
        let (challenge, auth_state) = self.webauthn.start_passkey_authentication(&passkeys)?;

        Ok((challenge, auth_state, shopify_customer_id))
    }

    /// Finish passkey authentication for a Shopify customer.
    ///
    /// Validates the client's response and updates credentials if needed.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::WebAuthn` if validation fails.
    pub async fn finish_passkey_authentication_for_shopify_customer(
        &self,
        state: &PasskeyAuthentication,
        response: &PublicKeyCredential,
        _shopify_customer_id: &str,
    ) -> Result<(), AuthError> {
        // Verify the authentication
        let auth_result = self
            .webauthn
            .finish_passkey_authentication(response, state)?;

        // Update credential if needed
        if auth_result.needs_update() {
            let cred_id = auth_result.cred_id();
            if let Some(mut credential) = self
                .users
                .get_credential_by_webauthn_id(cred_id.as_ref())
                .await?
            {
                credential.passkey.update_credential(&auth_result);
                self.users
                    .update_credential(cred_id.as_ref(), &credential.passkey)
                    .await?;
            }
        }

        Ok(())
    }

    /// Delete a credential for a Shopify customer.
    ///
    /// # Returns
    ///
    /// Returns `true` if the credential was deleted, `false` if it didn't exist.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::Repository` if the database operation fails.
    pub async fn delete_credential_for_shopify_customer(
        &self,
        shopify_customer_id: &str,
        credential_id: naked_pineapple_core::CredentialId,
    ) -> Result<bool, AuthError> {
        let deleted = self
            .users
            .delete_credential_for_shopify_customer(shopify_customer_id, credential_id)
            .await?;
        Ok(deleted)
    }
}

/// Generate a UUID from a Shopify customer ID.
///
/// This creates a deterministic UUID from the customer ID so that
/// the same customer always gets the same UUID for `WebAuthn` purposes.
fn uuid_from_shopify_customer_id(customer_id: &str) -> Uuid {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    customer_id.hash(&mut hasher);
    let hash = hasher.finish();

    // Create a UUID from the hash (using a namespace-like approach)
    let bytes = hash.to_le_bytes();
    let mut uuid_bytes = [0u8; 16];
    uuid_bytes[..8].copy_from_slice(&bytes);
    uuid_bytes[8..].copy_from_slice(&bytes);

    // Set version 4 (random) and variant bits
    uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x40; // Version 4
    uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80; // Variant 1

    Uuid::from_bytes(uuid_bytes)
}

/// Validate password meets requirements.
fn validate_password(password: &str) -> Result<(), AuthError> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(AuthError::WeakPassword(format!(
            "password must be at least {MIN_PASSWORD_LENGTH} characters"
        )));
    }

    // Add more validation as needed (uppercase, numbers, symbols, etc.)

    Ok(())
}

/// Hash a password using Argon2id.
fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::PasswordHash)
}

/// Verify a password against a hash.
fn verify_password(password: &str, hash: &str) -> Result<(), AuthError> {
    let parsed_hash = PasswordHash::new(hash).map_err(|_| AuthError::InvalidCredentials)?;
    let argon2 = Argon2::default();

    argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| AuthError::InvalidCredentials)
}

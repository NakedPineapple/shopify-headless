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
use tracing::{debug, info, instrument, warn};
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
    #[instrument(skip(self, password), fields(email = %email))]
    pub async fn register_with_password(
        &self,
        email: &str,
        password: &str,
    ) -> Result<User, AuthError> {
        debug!("Starting user registration with password");

        // Validate email
        let email = Email::parse(email)?;

        // Validate password
        validate_password(password)?;

        // Hash password
        let password_hash = hash_password(password)?;

        // Create user
        let result = self
            .users
            .create_with_password(&email, &password_hash)
            .await
            .map_err(|e| match e {
                RepositoryError::Conflict(_) => AuthError::UserAlreadyExists,
                other => AuthError::Repository(other),
            });

        match &result {
            Ok(user) => info!(user_id = %user.id, "User registered successfully with password"),
            Err(AuthError::UserAlreadyExists) => {
                warn!("Registration failed: user already exists");
            }
            Err(e) => warn!(error = %e, "Registration failed"),
        }

        result
    }

    /// Login with email and password.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::InvalidCredentials` if the email/password is wrong.
    #[instrument(skip(self, password), fields(email = %email))]
    pub async fn login_with_password(
        &self,
        email: &str,
        password: &str,
    ) -> Result<User, AuthError> {
        debug!("Attempting password login");

        // Validate email format
        let email = Email::parse(email)?;

        // Get user with password hash
        let (user, password_hash) =
            self.users.get_password_hash(&email).await?.ok_or_else(|| {
                warn!("Login failed: user not found");
                AuthError::InvalidCredentials
            })?;

        // Verify password
        if let Err(e) = verify_password(password, &password_hash) {
            warn!("Login failed: invalid password");
            return Err(e);
        }

        info!(user_id = %user.id, "User logged in successfully with password");
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
    #[instrument(skip(self, existing_credentials), fields(user_id = %user.id, email = %user.email.as_str()))]
    pub fn start_passkey_registration(
        &self,
        user: &User,
        existing_credentials: &[UserCredential],
    ) -> Result<(CreationChallengeResponse, PasskeyRegistration), AuthError> {
        info!(
            existing_credentials_count = existing_credentials.len(),
            "Starting passkey registration"
        );

        // Collect existing credential IDs to exclude
        let exclude_credentials: Vec<CredentialID> = existing_credentials
            .iter()
            .map(|c| CredentialID::from(c.webauthn_id.clone()))
            .collect();

        // Create challenge
        let result = self.webauthn.start_passkey_registration(
            Uuid::new_v4(),
            user.email.as_str(),
            user.email.as_str(),
            Some(exclude_credentials),
        );

        match &result {
            Ok(_) => debug!("Passkey registration challenge created"),
            Err(e) => warn!(error = %e, "Failed to create passkey registration challenge"),
        }

        Ok(result?)
    }

    /// Finish passkey registration.
    ///
    /// Validates the client's response and returns the passkey to store.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::WebAuthn` if validation fails.
    #[instrument(skip(self, state, response))]
    pub fn finish_passkey_registration(
        &self,
        state: &PasskeyRegistration,
        response: &RegisterPublicKeyCredential,
    ) -> Result<Passkey, AuthError> {
        debug!("Finishing passkey registration");

        let result = self.webauthn.finish_passkey_registration(response, state);

        match &result {
            Ok(_) => info!("Passkey registration completed successfully"),
            Err(e) => warn!(error = %e, "Passkey registration validation failed"),
        }

        Ok(result?)
    }

    /// Save a registered credential to the database.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::Repository` if the database operation fails.
    #[instrument(skip(self, passkey), fields(user_id = %user_id, credential_name = %name))]
    pub async fn save_credential(
        &self,
        user_id: UserId,
        passkey: &Passkey,
        name: &str,
    ) -> Result<UserCredential, AuthError> {
        debug!("Saving credential to database");

        let result = self.users.create_credential(user_id, passkey, name).await;

        match &result {
            Ok(credential) => {
                info!(credential_id = %credential.id, "Credential saved successfully");
            }
            Err(e) => warn!(error = %e, "Failed to save credential"),
        }

        Ok(result?)
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
    #[instrument(skip(self), fields(email = %email))]
    pub async fn start_passkey_authentication(
        &self,
        email: &str,
    ) -> Result<(RequestChallengeResponse, PasskeyAuthentication, UserId), AuthError> {
        debug!("Starting passkey authentication");

        // Validate and find user
        let email = Email::parse(email)?;
        let user = self.users.get_by_email(&email).await?.ok_or_else(|| {
            warn!("Passkey authentication failed: user not found");
            AuthError::UserNotFound
        })?;

        // Get credentials
        let credentials = self.users.get_credentials(user.id).await?;
        if credentials.is_empty() {
            warn!(user_id = %user.id, "Passkey authentication failed: no credentials registered");
            return Err(AuthError::NoCredentials);
        }

        debug!(
            user_id = %user.id,
            credentials_count = credentials.len(),
            "Found credentials for passkey authentication"
        );

        // Get passkeys for WebAuthn
        let passkeys: Vec<Passkey> = credentials.iter().map(|c| c.passkey.clone()).collect();

        // Create challenge
        let (challenge, auth_state) = self.webauthn.start_passkey_authentication(&passkeys)?;

        info!(user_id = %user.id, "Passkey authentication challenge created");
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
    #[instrument(skip(self, state, response), fields(user_id = %user_id))]
    pub async fn finish_passkey_authentication(
        &self,
        state: &PasskeyAuthentication,
        response: &PublicKeyCredential,
        user_id: UserId,
    ) -> Result<User, AuthError> {
        debug!("Finishing passkey authentication");

        // Verify the authentication
        let auth_result = self
            .webauthn
            .finish_passkey_authentication(response, state)
            .map_err(|e| {
                warn!(error = %e, "Passkey authentication validation failed");
                e
            })?;

        // Update credential if needed
        if auth_result.needs_update() {
            debug!("Credential needs update, updating stored passkey");
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
                debug!("Credential updated successfully");
            }
        }

        // Get the user
        let user = self.users.get_by_id(user_id).await?.ok_or_else(|| {
            warn!("Passkey authentication failed: user not found after validation");
            AuthError::UserNotFound
        })?;

        info!("User authenticated successfully via passkey");
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
    #[instrument(skip(self), fields(user_id = %user_id))]
    pub async fn get_credentials(&self, user_id: UserId) -> Result<Vec<UserCredential>, AuthError> {
        debug!("Fetching credentials for user");
        let credentials = self.users.get_credentials(user_id).await?;
        debug!(
            credentials_count = credentials.len(),
            "Retrieved credentials"
        );
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
    #[instrument(skip(self), fields(user_id = %user_id, credential_id = %credential_id))]
    pub async fn delete_credential(
        &self,
        user_id: UserId,
        credential_id: naked_pineapple_core::CredentialId,
    ) -> Result<bool, AuthError> {
        debug!("Deleting credential");
        let deleted = self.users.delete_credential(user_id, credential_id).await?;
        if deleted {
            info!("Credential deleted successfully");
        } else {
            debug!("Credential not found for deletion");
        }
        Ok(deleted)
    }

    /// Get a user by ID.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::UserNotFound` if the user doesn't exist.
    #[instrument(skip(self), fields(user_id = %user_id))]
    pub async fn get_user(&self, user_id: UserId) -> Result<User, AuthError> {
        debug!("Fetching user by ID");
        self.users.get_by_id(user_id).await?.ok_or_else(|| {
            warn!("User not found");
            AuthError::UserNotFound
        })
    }

    // =========================================================================
    // Shopify Customer WebAuthn Methods
    // =========================================================================

    /// Get all credentials for a Shopify customer.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::Repository` if the database operation fails.
    #[instrument(skip(self), fields(shopify_customer_id = %shopify_customer_id))]
    pub async fn get_credentials_by_shopify_customer_id(
        &self,
        shopify_customer_id: &str,
    ) -> Result<Vec<UserCredential>, AuthError> {
        debug!("Fetching credentials for Shopify customer");
        let credentials = self
            .users
            .get_credentials_by_shopify_customer_id(shopify_customer_id)
            .await?;
        debug!(
            credentials_count = credentials.len(),
            "Retrieved Shopify customer credentials"
        );
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
    #[instrument(skip(self, existing_credentials), fields(shopify_customer_id = %shopify_customer_id, email = %email))]
    pub fn start_passkey_registration_for_shopify_customer(
        &self,
        shopify_customer_id: &str,
        email: &str,
        existing_credentials: &[UserCredential],
    ) -> Result<(CreationChallengeResponse, PasskeyRegistration), AuthError> {
        info!(
            existing_credentials_count = existing_credentials.len(),
            "Starting passkey registration for Shopify customer"
        );

        // Collect existing credential IDs to exclude
        let exclude_credentials: Vec<CredentialID> = existing_credentials
            .iter()
            .map(|c| CredentialID::from(c.webauthn_id.clone()))
            .collect();

        // Use Shopify customer ID as the user UUID (hash it to get a consistent UUID)
        let user_uuid = uuid_from_shopify_customer_id(shopify_customer_id);

        // Create challenge
        let result = self.webauthn.start_passkey_registration(
            user_uuid,
            email,
            email,
            Some(exclude_credentials),
        );

        match &result {
            Ok(_) => debug!("Passkey registration challenge created for Shopify customer"),
            Err(e) => {
                warn!(error = %e, "Failed to create passkey registration challenge for Shopify customer");
            }
        }

        Ok(result?)
    }

    /// Save a registered credential for a Shopify customer.
    ///
    /// The email is stored to enable passkey-by-email lookup for passwordless authentication.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::Repository` if the database operation fails.
    #[instrument(skip(self, passkey), fields(shopify_customer_id = %shopify_customer_id, email = %email.as_str(), credential_name = %name))]
    pub async fn save_credential_for_shopify_customer(
        &self,
        shopify_customer_id: &str,
        email: &Email,
        passkey: &Passkey,
        name: &str,
    ) -> Result<UserCredential, AuthError> {
        debug!("Saving credential for Shopify customer");

        let result = self
            .users
            .create_credential_for_shopify_customer(shopify_customer_id, email, passkey, name)
            .await;

        match &result {
            Ok(credential) => {
                info!(credential_id = %credential.id, "Credential saved for Shopify customer");
            }
            Err(e) => warn!(error = %e, "Failed to save credential for Shopify customer"),
        }

        Ok(result?)
    }

    /// Start discoverable passkey authentication for a Shopify customer.
    ///
    /// Uses discoverable credentials so no email is needed — the browser
    /// presents all saved passkeys for this relying party.
    ///
    /// # Errors
    ///
    /// Returns `AuthError::WebAuthn` if the challenge cannot be generated.
    #[instrument(skip(self))]
    pub fn start_discoverable_authentication_for_shopify_customer(
        &self,
    ) -> Result<(RequestChallengeResponse, DiscoverableAuthentication), AuthError> {
        debug!("Starting discoverable passkey authentication for Shopify customer");

        let (challenge, auth_state) = self.webauthn.start_discoverable_authentication()?;

        info!("Discoverable passkey authentication challenge created");
        Ok((challenge, auth_state))
    }

    /// Finish discoverable passkey authentication for a Shopify customer.
    ///
    /// Looks up the credential by its `WebAuthn` ID from the response, verifies
    /// the authentication, and returns the matched credential (which contains
    /// the `shopify_customer_id` and email).
    ///
    /// # Errors
    ///
    /// Returns `AuthError::CredentialNotFound` if the credential isn't found.
    /// Returns `AuthError::WebAuthn` if validation fails.
    #[instrument(skip(self, state, response))]
    pub async fn finish_discoverable_authentication_for_shopify_customer(
        &self,
        state: DiscoverableAuthentication,
        response: &PublicKeyCredential,
    ) -> Result<UserCredential, AuthError> {
        debug!("Finishing discoverable passkey authentication for Shopify customer");

        // Look up the credential by its WebAuthn ID from the response
        let credential = self
            .users
            .get_credential_by_webauthn_id(response.raw_id.as_ref())
            .await?
            .ok_or(AuthError::CredentialNotFound)?;

        // Convert to DiscoverableKey for verification
        let discoverable_key: DiscoverableKey = credential.passkey.clone().into();

        // Verify the authentication
        let auth_result = self
            .webauthn
            .finish_discoverable_authentication(response, state, &[discoverable_key])
            .map_err(|e| {
                warn!(error = %e, "Discoverable authentication validation failed");
                e
            })?;

        // Update credential if needed
        if auth_result.needs_update() {
            debug!("Credential needs update, updating stored passkey");
            let cred_id = auth_result.cred_id();
            if let Some(mut cred) = self
                .users
                .get_credential_by_webauthn_id(cred_id.as_ref())
                .await?
            {
                cred.passkey.update_credential(&auth_result);
                self.users
                    .update_credential(cred_id.as_ref(), &cred.passkey)
                    .await?;
            }
        }

        info!(
            shopify_customer_id = %credential.shopify_customer_id,
            "Shopify customer authenticated successfully via discoverable passkey"
        );
        Ok(credential)
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
    #[instrument(skip(self), fields(shopify_customer_id = %shopify_customer_id, credential_id = %credential_id))]
    pub async fn delete_credential_for_shopify_customer(
        &self,
        shopify_customer_id: &str,
        credential_id: naked_pineapple_core::CredentialId,
    ) -> Result<bool, AuthError> {
        debug!("Deleting credential for Shopify customer");
        let deleted = self
            .users
            .delete_credential_for_shopify_customer(shopify_customer_id, credential_id)
            .await?;
        if deleted {
            info!("Credential deleted successfully for Shopify customer");
        } else {
            debug!("Credential not found for Shopify customer deletion");
        }
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

/// Maximum password length to prevent denial of service via hashing.
const MAX_PASSWORD_LENGTH: usize = 128;

/// Validate password meets security requirements.
///
/// Requirements:
/// - At least 8 characters
/// - At most 128 characters
/// - At least one uppercase letter
/// - At least one lowercase letter
/// - At least one digit
fn validate_password(password: &str) -> Result<(), AuthError> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(AuthError::WeakPassword(format!(
            "password must be at least {MIN_PASSWORD_LENGTH} characters"
        )));
    }

    if password.len() > MAX_PASSWORD_LENGTH {
        return Err(AuthError::WeakPassword(format!(
            "password must be {MAX_PASSWORD_LENGTH} characters or less"
        )));
    }

    // Check for at least one uppercase letter
    if !password.chars().any(|c| c.is_ascii_uppercase()) {
        return Err(AuthError::WeakPassword(
            "password must contain at least one uppercase letter".to_string(),
        ));
    }

    // Check for at least one lowercase letter
    if !password.chars().any(|c| c.is_ascii_lowercase()) {
        return Err(AuthError::WeakPassword(
            "password must contain at least one lowercase letter".to_string(),
        ));
    }

    // Check for at least one digit
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(AuthError::WeakPassword(
            "password must contain at least one number".to_string(),
        ));
    }

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

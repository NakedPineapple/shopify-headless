//! Admin authentication service.
//!
//! Provides `WebAuthn` passkey-only authentication for admin panel.
//! No password authentication is supported - only passkeys.
//!
//! Uses discoverable credentials (resident keys) to enable login without email input.

mod error;

pub use error::AdminAuthError;

use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;
use webauthn_rs::prelude::*;

use naked_pineapple_core::{AdminCredentialId, AdminUserId};

use crate::db::RepositoryError;
use crate::db::admin_users::AdminUserRepository;
use crate::models::admin_user::{AdminCredential, AdminUser};

/// Admin authentication service.
///
/// Handles `WebAuthn` passkey authentication for admin users.
/// No password authentication is supported.
pub struct AdminAuthService<'a> {
    users: AdminUserRepository<'a>,
    webauthn: &'a Webauthn,
}

impl<'a> AdminAuthService<'a> {
    /// Create a new admin authentication service.
    #[must_use]
    pub const fn new(pool: &'a PgPool, webauthn: &'a Webauthn) -> Self {
        Self {
            users: AdminUserRepository::new(pool),
            webauthn,
        }
    }

    // =========================================================================
    // WebAuthn Registration (Discoverable Credentials)
    // =========================================================================

    /// Start discoverable passkey registration for an existing admin user.
    ///
    /// Discoverable credentials (resident keys) store the user handle on the authenticator,
    /// enabling login without email input.
    ///
    /// Returns the challenge to send to the client and the registration state
    /// to store in the session.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::WebAuthn` if the challenge cannot be generated.
    pub fn start_passkey_registration(
        &self,
        user: &AdminUser,
        existing_credentials: &[AdminCredential],
    ) -> Result<(CreationChallengeResponse, PasskeyRegistration), AdminAuthError> {
        // Collect existing credential IDs to exclude
        let exclude_credentials: Vec<CredentialID> = existing_credentials
            .iter()
            .map(|c| CredentialID::from(c.webauthn_id.clone()))
            .collect();

        // Create challenge using the user's persistent webauthn_user_id
        // This UUID is stored in the passkey and returned during authentication
        let (challenge, reg_state) = self.webauthn.start_passkey_registration(
            user.webauthn_user_id,
            user.email.as_str(),
            user.name.as_str(),
            Some(exclude_credentials),
        )?;

        Ok((challenge, reg_state))
    }

    /// Start discoverable passkey registration for a new admin (during setup).
    ///
    /// The `webauthn_user_id` should be generated and stored so it can be used
    /// when creating the admin user after registration completes.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::WebAuthn` if the challenge cannot be generated.
    pub fn start_passkey_registration_for_new_user(
        &self,
        webauthn_user_id: Uuid,
        email: &str,
        display_name: &str,
    ) -> Result<(CreationChallengeResponse, PasskeyRegistration), AdminAuthError> {
        let (challenge, reg_state) = self.webauthn.start_passkey_registration(
            webauthn_user_id,
            email,
            display_name,
            None, // No existing credentials for new user
        )?;

        Ok((challenge, reg_state))
    }

    /// Finish passkey registration.
    ///
    /// Validates the client's response and returns the passkey to store.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::WebAuthn` if validation fails.
    pub fn finish_passkey_registration(
        &self,
        state: &PasskeyRegistration,
        response: &RegisterPublicKeyCredential,
    ) -> Result<Passkey, AdminAuthError> {
        let passkey = self.webauthn.finish_passkey_registration(response, state)?;
        Ok(passkey)
    }

    /// Save a registered credential to the database.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::Repository` if the database operation fails.
    pub async fn save_credential(
        &self,
        admin_user_id: AdminUserId,
        passkey: &Passkey,
        name: &str,
    ) -> Result<AdminCredential, AdminAuthError> {
        let credential = self
            .users
            .create_credential(admin_user_id, passkey, name)
            .await?;
        Ok(credential)
    }

    // =========================================================================
    // WebAuthn Authentication (Discoverable)
    // =========================================================================

    /// Start discoverable passkey authentication.
    ///
    /// This does NOT require knowing the user upfront - the authenticator will
    /// present available credentials and return the user handle.
    ///
    /// Returns the challenge to send to the client and the authentication state
    /// to store in the session.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::NoCredentials` if no admin passkeys exist.
    /// Returns `AdminAuthError::WebAuthn` if the challenge cannot be generated.
    pub async fn start_passkey_authentication(
        &self,
    ) -> Result<(RequestChallengeResponse, DiscoverableAuthentication), AdminAuthError> {
        // Get all credentials to verify we have at least one admin
        let credentials = self.users.get_all_credentials().await?;
        if credentials.is_empty() {
            return Err(AdminAuthError::NoCredentials);
        }

        // Start discoverable authentication - no credentials needed upfront
        let (challenge, auth_state) = self.webauthn.start_discoverable_authentication()?;

        Ok((challenge, auth_state))
    }

    /// Finish discoverable passkey authentication.
    ///
    /// Extracts the user handle from the credential response, looks up the user,
    /// and validates the authentication.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::WebAuthn` if validation fails.
    /// Returns `AdminAuthError::UserNotFound` if the user handle doesn't match any admin.
    /// Returns `AdminAuthError::CredentialNotFound` if the credential isn't found.
    pub async fn finish_passkey_authentication(
        &self,
        state: &DiscoverableAuthentication,
        response: &PublicKeyCredential,
    ) -> Result<AdminUser, AdminAuthError> {
        // Extract the user handle from the credential response
        // For discoverable credentials, this contains the webauthn_user_id
        let user_handle = response
            .response
            .user_handle
            .as_ref()
            .ok_or(AdminAuthError::InvalidUserHandle)?;

        // Parse the user handle as UUID (webauthn_user_id)
        let webauthn_user_id = Uuid::from_slice(user_handle.as_ref())
            .map_err(|_| AdminAuthError::InvalidUserHandle)?;

        // Get all credentials to find the matching one
        let credentials = self.users.get_all_credentials().await?;
        let passkeys: Vec<DiscoverableKey> = credentials
            .iter()
            .map(|c| c.passkey.clone().into())
            .collect();

        // Verify the authentication
        let auth_result =
            self.webauthn
                .finish_discoverable_authentication(response, state.clone(), &passkeys)?;

        // Look up the admin user by their webauthn_user_id
        let user = self
            .users
            .get_by_webauthn_user_id(webauthn_user_id)
            .await?
            .ok_or(AdminAuthError::UserNotFound)?;

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

        Ok(user)
    }

    // =========================================================================
    // Break-Glass Password Authentication
    // =========================================================================

    /// Authenticate an admin user with email and password (break-glass only).
    ///
    /// Returns the authenticated `AdminUser` on success.
    /// Performs a dummy hash verification when the user is not found or has no
    /// password to prevent timing-based user enumeration.
    ///
    /// # Security
    ///
    /// This method logs at `WARN` level for all attempts (success and failure)
    /// because password login is an emergency mechanism and should be audited.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::InvalidCredentials` for wrong email or password.
    /// Returns `AdminAuthError::PasswordNotSet` if the user has no password.
    pub async fn authenticate_with_password(
        &self,
        email: &str,
        password: &SecretString,
    ) -> Result<AdminUser, AdminAuthError> {
        let email = naked_pineapple_core::Email::parse(email)?;

        let result = self.users.get_with_password_hash(&email).await?;

        let Some((user, hash)) = result else {
            // User not found -- do a dummy hash to prevent timing leaks
            Self::dummy_verify(password);
            warn!(email = %email.as_str(), "Break-glass password login FAILED: user not found");
            return Err(AdminAuthError::InvalidCredentials);
        };

        let Some(hash) = hash else {
            // User exists but has no password set
            Self::dummy_verify(password);
            warn!(
                user_id = %user.id.as_i32(),
                email = %email.as_str(),
                "Break-glass password login FAILED: no password set"
            );
            return Err(AdminAuthError::PasswordNotSet);
        };

        if !Self::verify_password(password, &hash)? {
            warn!(
                user_id = %user.id.as_i32(),
                email = %email.as_str(),
                "Break-glass password login FAILED: wrong password"
            );
            return Err(AdminAuthError::InvalidCredentials);
        }

        warn!(
            user_id = %user.id.as_i32(),
            email = %email.as_str(),
            "Break-glass password login SUCCEEDED - emergency auth used"
        );

        Ok(user)
    }

    /// Set a break-glass password for an admin user.
    ///
    /// The password is hashed with Argon2id before storage.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::PasswordTooShort` if under 12 characters.
    /// Returns `AdminAuthError::PasswordHash` if hashing fails.
    /// Returns `AdminAuthError::Repository` for database errors.
    pub async fn set_password(
        &self,
        target_user_id: AdminUserId,
        password: &SecretString,
    ) -> Result<(), AdminAuthError> {
        if password.expose_secret().len() < 12 {
            return Err(AdminAuthError::PasswordTooShort);
        }

        let hash = Self::hash_password(password)?;
        self.users
            .set_password_hash(target_user_id, Some(&hash))
            .await?;

        info!(
            target_user_id = %target_user_id.as_i32(),
            "Break-glass password SET for admin user"
        );

        Ok(())
    }

    /// Remove a break-glass password for an admin user.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::Repository` for database errors.
    pub async fn clear_password(&self, target_user_id: AdminUserId) -> Result<(), AdminAuthError> {
        self.users.set_password_hash(target_user_id, None).await?;

        info!(
            target_user_id = %target_user_id.as_i32(),
            "Break-glass password CLEARED for admin user"
        );

        Ok(())
    }

    /// Hash a password using Argon2id with a random salt.
    fn hash_password(password: &SecretString) -> Result<String, AdminAuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.expose_secret().as_bytes(), &salt)
            .map_err(|e| AdminAuthError::PasswordHash(e.to_string()))?;
        Ok(hash.to_string())
    }

    /// Verify a password against a stored Argon2id hash.
    fn verify_password(password: &SecretString, hash: &str) -> Result<bool, AdminAuthError> {
        let parsed =
            PasswordHash::new(hash).map_err(|e| AdminAuthError::PasswordHash(e.to_string()))?;
        Ok(Argon2::default()
            .verify_password(password.expose_secret().as_bytes(), &parsed)
            .is_ok())
    }

    /// Perform a dummy password hash to prevent timing leaks when user is not found.
    fn dummy_verify(password: &SecretString) {
        // Use a fixed, valid PHC-format hash for constant-time comparison
        let dummy = "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        if let Ok(parsed) = PasswordHash::new(dummy) {
            let _ = Argon2::default().verify_password(password.expose_secret().as_bytes(), &parsed);
        }
    }

    // =========================================================================
    // Credential Management
    // =========================================================================

    /// Get all credentials for an admin user.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::Repository` if the database operation fails.
    pub async fn get_credentials(
        &self,
        admin_user_id: AdminUserId,
    ) -> Result<Vec<AdminCredential>, AdminAuthError> {
        let credentials = self.users.get_credentials(admin_user_id).await?;
        Ok(credentials)
    }

    /// Get an admin user by ID.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::UserNotFound` if the user doesn't exist.
    pub async fn get_user(&self, admin_user_id: AdminUserId) -> Result<AdminUser, AdminAuthError> {
        self.users
            .get_by_id(admin_user_id)
            .await?
            .ok_or(AdminAuthError::UserNotFound)
    }

    /// Get an admin user by email.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::UserNotFound` if the user doesn't exist.
    pub async fn get_user_by_email(&self, email: &str) -> Result<AdminUser, AdminAuthError> {
        use naked_pineapple_core::Email;

        let email = Email::parse(email)?;
        self.users
            .get_by_email(&email)
            .await?
            .ok_or(AdminAuthError::UserNotFound)
    }

    /// Delete a credential, preventing deletion of the last one.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::LastCredential` if this is the user's only passkey.
    /// Returns `AdminAuthError::CredentialNotFound` if the credential doesn't exist
    /// or doesn't belong to the user.
    /// Returns `AdminAuthError::Repository` for database errors.
    pub async fn delete_credential(
        &self,
        admin_user_id: AdminUserId,
        credential_id: AdminCredentialId,
    ) -> Result<(), AdminAuthError> {
        // Check this isn't the last credential
        let count = self.users.count_credentials(admin_user_id).await?;
        if count <= 1 {
            return Err(AdminAuthError::LastCredential);
        }

        // Delete the credential (with ownership verification)
        self.users
            .delete_credential(credential_id, admin_user_id)
            .await
            .map_err(|e| match e {
                crate::db::RepositoryError::NotFound => AdminAuthError::CredentialNotFound,
                other => AdminAuthError::Repository(other),
            })?;

        Ok(())
    }
}

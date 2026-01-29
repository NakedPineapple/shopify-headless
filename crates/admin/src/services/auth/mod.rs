//! Admin authentication service.
//!
//! Provides `WebAuthn` passkey-only authentication for admin panel.
//! No password authentication is supported - only passkeys.
//!
//! Uses discoverable credentials (resident keys) to enable login without email input.

mod error;

pub use error::AdminAuthError;

use sqlx::PgPool;
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

//! Admin authentication service.
//!
//! Provides `WebAuthn` passkey-only authentication for admin panel.
//! No password authentication is supported - only passkeys.

mod error;

pub use error::AdminAuthError;

use sqlx::PgPool;
use uuid::Uuid;
use webauthn_rs::prelude::*;

use naked_pineapple_core::AdminUserId;

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
    // WebAuthn Registration
    // =========================================================================

    /// Start passkey registration for an existing admin user.
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
    // WebAuthn Authentication
    // =========================================================================

    /// Start passkey authentication for an admin user.
    ///
    /// Returns the challenge to send to the client and the authentication state
    /// to store in the session.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::UserNotFound` if the user doesn't exist.
    /// Returns `AdminAuthError::NoCredentials` if the user has no registered passkeys.
    /// Returns `AdminAuthError::WebAuthn` if the challenge cannot be generated.
    pub async fn start_passkey_authentication(
        &self,
        email: &str,
    ) -> Result<(RequestChallengeResponse, PasskeyAuthentication, AdminUserId), AdminAuthError>
    {
        use naked_pineapple_core::Email;

        // Validate and find user
        let email = Email::parse(email)?;
        let user = self
            .users
            .get_by_email(&email)
            .await?
            .ok_or(AdminAuthError::UserNotFound)?;

        // Get credentials
        let credentials = self.users.get_credentials(user.id).await?;
        if credentials.is_empty() {
            return Err(AdminAuthError::NoCredentials);
        }

        // Get passkeys for WebAuthn
        let passkeys: Vec<Passkey> = credentials.iter().map(|c| c.passkey.clone()).collect();

        // Create challenge
        let (challenge, auth_state) = self.webauthn.start_passkey_authentication(&passkeys)?;

        Ok((challenge, auth_state, user.id))
    }

    /// Finish passkey authentication.
    ///
    /// Validates the client's response and returns the authenticated admin user.
    ///
    /// # Errors
    ///
    /// Returns `AdminAuthError::WebAuthn` if validation fails.
    /// Returns `AdminAuthError::CredentialNotFound` if the credential isn't found.
    pub async fn finish_passkey_authentication(
        &self,
        state: &PasskeyAuthentication,
        response: &PublicKeyCredential,
        admin_user_id: AdminUserId,
    ) -> Result<AdminUser, AdminAuthError> {
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

        // Get the admin user
        let user = self
            .users
            .get_by_id(admin_user_id)
            .await?
            .ok_or(AdminAuthError::UserNotFound)?;

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
}

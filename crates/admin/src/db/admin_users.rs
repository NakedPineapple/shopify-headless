//! Admin user repository for database operations.
//!
//! This module provides database access for admin users and their `WebAuthn` credentials.
//! All queries use sqlx macros for compile-time verification.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;
use webauthn_rs::prelude::Passkey;

use naked_pineapple_core::{AdminCredentialId, AdminUserId, Email};

use super::RepositoryError;
use crate::models::admin_user::{AdminCredential, AdminRole, AdminUser};

// =============================================================================
// Internal Row Types
// =============================================================================

/// Internal row type for `PostgreSQL` admin user queries.
#[derive(Debug, sqlx::FromRow)]
struct AdminUserRow {
    id: i32,
    email: String,
    name: String,
    role: AdminRole,
    webauthn_user_id: Uuid,
    slack_user_id: Option<String>,
    password_hash: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<AdminUserRow> for AdminUser {
    type Error = RepositoryError;

    fn try_from(row: AdminUserRow) -> Result<Self, Self::Error> {
        let email = Email::parse(&row.email).map_err(|e| {
            RepositoryError::DataCorruption(format!("invalid email in database: {e}"))
        })?;

        Ok(Self {
            id: AdminUserId::new(row.id),
            email,
            name: row.name,
            role: row.role,
            webauthn_user_id: row.webauthn_user_id,
            slack_user_id: row.slack_user_id,
            has_password: row.password_hash.is_some(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

/// Internal row type for `PostgreSQL` admin credential queries.
#[derive(Debug, sqlx::FromRow)]
struct AdminCredentialRow {
    id: i32,
    admin_user_id: i32,
    credential_id: Vec<u8>,
    public_key: Vec<u8>,
    name: String,
    created_at: DateTime<Utc>,
}

impl TryFrom<AdminCredentialRow> for AdminCredential {
    type Error = RepositoryError;

    fn try_from(row: AdminCredentialRow) -> Result<Self, Self::Error> {
        let passkey: Passkey = serde_json::from_slice(&row.public_key)
            .map_err(|e| RepositoryError::DataCorruption(format!("invalid passkey data: {e}")))?;

        Ok(Self {
            id: AdminCredentialId::new(row.id),
            admin_user_id: AdminUserId::new(row.admin_user_id),
            webauthn_id: row.credential_id,
            passkey,
            name: row.name,
            created_at: row.created_at,
        })
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for admin user database operations.
pub struct AdminUserRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> AdminUserRepository<'a> {
    /// Create a new admin user repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// List all admin users.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if the data is invalid.
    #[instrument(skip(self), level = "debug")]
    pub async fn list_all(&self) -> Result<Vec<AdminUser>, RepositoryError> {
        debug!("Listing all admin users");
        let rows = sqlx::query_as!(
            AdminUserRow,
            r#"
            SELECT id, email, name, role as "role: AdminRole",
                   webauthn_user_id, slack_user_id, password_hash,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM admin.admin_user
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(self.pool)
        .await?;

        debug!(count = rows.len(), "Found admin users");
        rows.into_iter().map(TryInto::try_into).collect()
    }

    /// Get an admin user by their ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if the data is invalid.
    #[instrument(skip(self), fields(id = %id.as_i32()), level = "debug")]
    pub async fn get_by_id(&self, id: AdminUserId) -> Result<Option<AdminUser>, RepositoryError> {
        debug!("Fetching admin user by ID");
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            SELECT id, email, name, role as "role: AdminRole",
                   webauthn_user_id, slack_user_id, password_hash,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM admin.admin_user
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?;

        if row.is_none() {
            debug!("Admin user not found");
        }
        row.map(TryInto::try_into).transpose()
    }

    /// Get an admin user by their email address.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if the data is invalid.
    #[instrument(skip(self), fields(email = %email.as_str()), level = "debug")]
    pub async fn get_by_email(&self, email: &Email) -> Result<Option<AdminUser>, RepositoryError> {
        debug!("Fetching admin user by email");
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            SELECT id, email, name, role as "role: AdminRole",
                   webauthn_user_id, slack_user_id, password_hash,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM admin.admin_user
            WHERE email = $1
            "#,
            email.as_str()
        )
        .fetch_optional(self.pool)
        .await?;

        if row.is_none() {
            debug!("Admin user not found");
        }
        row.map(TryInto::try_into).transpose()
    }

    /// Get an admin user by their `WebAuthn` user ID (for discoverable credential authentication).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if the data is invalid.
    #[instrument(skip(self), fields(webauthn_user_id = %webauthn_user_id), level = "debug")]
    pub async fn get_by_webauthn_user_id(
        &self,
        webauthn_user_id: Uuid,
    ) -> Result<Option<AdminUser>, RepositoryError> {
        debug!("Fetching admin user by WebAuthn user ID");
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            SELECT id, email, name, role as "role: AdminRole",
                   webauthn_user_id, slack_user_id, password_hash,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM admin.admin_user
            WHERE webauthn_user_id = $1
            "#,
            webauthn_user_id
        )
        .fetch_optional(self.pool)
        .await?;

        if row.is_none() {
            debug!("Admin user not found");
        }
        row.map(TryInto::try_into).transpose()
    }

    /// Create a new admin user.
    ///
    /// The `webauthn_user_id` is the UUID that will be stored in passkeys for discoverable
    /// credential authentication (login without email). This should be generated when
    /// starting passkey registration and passed here.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Conflict` if the email already exists.
    /// Returns `RepositoryError::Database` for other database errors.
    #[instrument(skip(self), fields(email = %email.as_str(), role = ?role), level = "debug")]
    pub async fn create(
        &self,
        email: &Email,
        name: &str,
        role: AdminRole,
        webauthn_user_id: Uuid,
    ) -> Result<AdminUser, RepositoryError> {
        debug!("Creating admin user");
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            INSERT INTO admin.admin_user (email, name, role, webauthn_user_id)
            VALUES ($1, $2, $3, $4)
            RETURNING id, email, name, role as "role: AdminRole",
                      webauthn_user_id, slack_user_id, password_hash,
                      created_at as "created_at: DateTime<Utc>",
                      updated_at as "updated_at: DateTime<Utc>"
            "#,
            email.as_str(),
            name,
            role as AdminRole,
            webauthn_user_id
        )
        .fetch_one(self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e
                && db_err.is_unique_violation()
            {
                warn!(email = %email.as_str(), "Admin user creation failed: email already exists");
                return RepositoryError::Conflict("email already exists".to_owned());
            }
            RepositoryError::Database(e)
        })?;

        let user: AdminUser = row.try_into()?;
        info!(user_id = %user.id.as_i32(), email = %user.email.as_str(), "Admin user created");
        Ok(user)
    }

    /// Get all credentials for an admin user.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if any credential data is invalid.
    #[instrument(skip(self), fields(admin_user_id = %admin_user_id.as_i32()), level = "debug")]
    pub async fn get_credentials(
        &self,
        admin_user_id: AdminUserId,
    ) -> Result<Vec<AdminCredential>, RepositoryError> {
        debug!("Fetching credentials for admin user");
        let rows = sqlx::query_as!(
            AdminCredentialRow,
            r#"
            SELECT id, admin_user_id, credential_id, public_key, name,
                   created_at as "created_at: DateTime<Utc>"
            FROM admin.admin_credential
            WHERE admin_user_id = $1
            ORDER BY created_at ASC
            "#,
            admin_user_id.as_i32()
        )
        .fetch_all(self.pool)
        .await?;

        debug!(count = rows.len(), "Found credentials for admin user");
        rows.into_iter().map(TryInto::try_into).collect()
    }

    /// Get all credentials from all admin users.
    ///
    /// Used for discoverable credential authentication where we don't know the user upfront.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if any credential data is invalid.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_all_credentials(&self) -> Result<Vec<AdminCredential>, RepositoryError> {
        debug!("Fetching all credentials from all admin users");
        let rows = sqlx::query_as!(
            AdminCredentialRow,
            r#"
            SELECT id, admin_user_id, credential_id, public_key, name,
                   created_at as "created_at: DateTime<Utc>"
            FROM admin.admin_credential
            ORDER BY created_at ASC
            "#
        )
        .fetch_all(self.pool)
        .await?;

        debug!(count = rows.len(), "Found credentials");
        rows.into_iter().map(TryInto::try_into).collect()
    }

    /// Create a new credential for an admin user.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Conflict` if the credential ID already exists.
    /// Returns `RepositoryError::Database` for other database errors.
    #[instrument(skip(self, passkey), fields(admin_user_id = %admin_user_id.as_i32()), level = "debug")]
    pub async fn create_credential(
        &self,
        admin_user_id: AdminUserId,
        passkey: &Passkey,
        name: &str,
    ) -> Result<AdminCredential, RepositoryError> {
        debug!("Creating credential for admin user");
        let public_key = serde_json::to_vec(passkey).map_err(|e| {
            RepositoryError::DataCorruption(format!("failed to serialize passkey: {e}"))
        })?;

        let row = sqlx::query_as!(
            AdminCredentialRow,
            r#"
            INSERT INTO admin.admin_credential (admin_user_id, credential_id, public_key, counter, name)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, admin_user_id, credential_id, public_key, name,
                      created_at as "created_at: DateTime<Utc>"
            "#,
            admin_user_id.as_i32(),
            passkey.cred_id().as_ref(),
            &public_key,
            0_i32,
            name
        )
        .fetch_one(self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e
                && db_err.is_unique_violation()
            {
                warn!(admin_user_id = %admin_user_id.as_i32(), "Credential creation failed: credential already exists");
                return RepositoryError::Conflict("credential already exists".to_owned());
            }
            RepositoryError::Database(e)
        })?;

        let credential: AdminCredential = row.try_into()?;
        info!(credential_id = %credential.id.as_i32(), admin_user_id = %admin_user_id.as_i32(), "Admin credential created");
        Ok(credential)
    }

    /// Get a credential by its `WebAuthn` credential ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if the credential data is invalid.
    #[instrument(skip(self, credential_id), level = "debug")]
    pub async fn get_credential_by_webauthn_id(
        &self,
        credential_id: &[u8],
    ) -> Result<Option<AdminCredential>, RepositoryError> {
        debug!("Fetching credential by WebAuthn credential ID");
        let row = sqlx::query_as!(
            AdminCredentialRow,
            r#"
            SELECT id, admin_user_id, credential_id, public_key, name,
                   created_at as "created_at: DateTime<Utc>"
            FROM admin.admin_credential
            WHERE credential_id = $1
            "#,
            credential_id
        )
        .fetch_optional(self.pool)
        .await?;

        if row.is_none() {
            debug!("Credential not found");
        }
        row.map(TryInto::try_into).transpose()
    }

    /// Update a credential's passkey data (after successful authentication).
    ///
    /// This updates the serialized passkey which includes the counter.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the credential doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    #[instrument(skip(self, credential_id, passkey), level = "debug")]
    pub async fn update_credential(
        &self,
        credential_id: &[u8],
        passkey: &Passkey,
    ) -> Result<(), RepositoryError> {
        debug!("Updating credential passkey data");
        let public_key = serde_json::to_vec(passkey).map_err(|e| {
            RepositoryError::DataCorruption(format!("failed to serialize passkey: {e}"))
        })?;

        let result = sqlx::query!(
            r#"
            UPDATE admin.admin_credential
            SET public_key = $1
            WHERE credential_id = $2
            "#,
            &public_key,
            credential_id
        )
        .execute(self.pool)
        .await?;

        if result.rows_affected() == 0 {
            debug!("Credential not found for update");
            return Err(RepositoryError::NotFound);
        }

        info!("Credential passkey data updated");
        Ok(())
    }

    /// Update an admin user's display name.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the user doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    #[instrument(skip(self), fields(id = %id.as_i32()), level = "debug")]
    pub async fn update_name(
        &self,
        id: AdminUserId,
        name: &str,
    ) -> Result<AdminUser, RepositoryError> {
        debug!("Updating admin user name");
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            UPDATE admin.admin_user
            SET name = $1
            WHERE id = $2
            RETURNING id, email, name, role as "role: AdminRole",
                      webauthn_user_id, slack_user_id, password_hash,
                      created_at as "created_at: DateTime<Utc>",
                      updated_at as "updated_at: DateTime<Utc>"
            "#,
            name,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?
        .ok_or_else(|| {
            debug!("Admin user not found for name update");
            RepositoryError::NotFound
        })?;

        let user: AdminUser = row.try_into()?;
        info!(user_id = %user.id.as_i32(), "Admin user name updated");
        Ok(user)
    }

    /// Update an admin user's email address.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the user doesn't exist.
    /// Returns `RepositoryError::Conflict` if the email is already used by another user.
    /// Returns `RepositoryError::Database` for other database errors.
    #[instrument(skip(self), fields(id = %id.as_i32(), email = %email.as_str()), level = "debug")]
    pub async fn update_email(
        &self,
        id: AdminUserId,
        email: &Email,
    ) -> Result<AdminUser, RepositoryError> {
        debug!("Updating admin user email");
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            UPDATE admin.admin_user
            SET email = $1
            WHERE id = $2
            RETURNING id, email, name, role as "role: AdminRole",
                      webauthn_user_id, slack_user_id, password_hash,
                      created_at as "created_at: DateTime<Utc>",
                      updated_at as "updated_at: DateTime<Utc>"
            "#,
            email.as_str(),
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e
                && db_err.is_unique_violation()
            {
                warn!(id = %id.as_i32(), email = %email.as_str(), "Admin user email update failed: email already exists");
                return RepositoryError::Conflict("email already exists".to_owned());
            }
            RepositoryError::Database(e)
        })?
        .ok_or_else(|| {
            debug!("Admin user not found for email update");
            RepositoryError::NotFound
        })?;

        let user: AdminUser = row.try_into()?;
        info!(user_id = %user.id.as_i32(), "Admin user email updated");
        Ok(user)
    }

    /// Count the number of credentials for an admin user.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(admin_user_id = %admin_user_id.as_i32()), level = "debug")]
    pub async fn count_credentials(
        &self,
        admin_user_id: AdminUserId,
    ) -> Result<i64, RepositoryError> {
        debug!("Counting credentials for admin user");
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM admin.admin_credential
            WHERE admin_user_id = $1
            "#,
            admin_user_id.as_i32()
        )
        .fetch_one(self.pool)
        .await?;

        debug!(count = count, "Credential count retrieved");
        Ok(count)
    }

    /// Delete a credential by its database ID.
    ///
    /// Verifies ownership before deletion.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the credential doesn't exist or doesn't
    /// belong to the specified user.
    /// Returns `RepositoryError::Database` for other database errors.
    #[instrument(skip(self), fields(credential_id = %credential_id.as_i32(), admin_user_id = %admin_user_id.as_i32()), level = "debug")]
    pub async fn delete_credential(
        &self,
        credential_id: AdminCredentialId,
        admin_user_id: AdminUserId,
    ) -> Result<(), RepositoryError> {
        debug!("Deleting credential");
        let result = sqlx::query!(
            r#"
            DELETE FROM admin.admin_credential
            WHERE id = $1 AND admin_user_id = $2
            "#,
            credential_id.as_i32(),
            admin_user_id.as_i32()
        )
        .execute(self.pool)
        .await?;

        if result.rows_affected() == 0 {
            debug!("Credential not found for deletion");
            return Err(RepositoryError::NotFound);
        }

        info!(credential_id = %credential_id.as_i32(), "Admin credential deleted");
        Ok(())
    }

    /// Update an admin user's role.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the user doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    #[instrument(skip(self), fields(id = %id.as_i32(), role = ?role), level = "debug")]
    pub async fn update_role(
        &self,
        id: AdminUserId,
        role: AdminRole,
    ) -> Result<AdminUser, RepositoryError> {
        debug!("Updating admin user role");
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            UPDATE admin.admin_user
            SET role = $1
            WHERE id = $2
            RETURNING id, email, name, role as "role: AdminRole",
                      webauthn_user_id, slack_user_id, password_hash,
                      created_at as "created_at: DateTime<Utc>",
                      updated_at as "updated_at: DateTime<Utc>"
            "#,
            role as AdminRole,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?
        .ok_or_else(|| {
            debug!("Admin user not found for role update");
            RepositoryError::NotFound
        })?;

        let user: AdminUser = row.try_into()?;
        info!(user_id = %user.id.as_i32(), role = ?role, "Admin user role updated");
        Ok(user)
    }

    /// Update an admin user's Slack user ID.
    ///
    /// Pass `None` to clear the Slack user ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the user doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    #[instrument(skip(self), fields(id = %id.as_i32()), level = "debug")]
    pub async fn update_slack_user_id(
        &self,
        id: AdminUserId,
        slack_user_id: Option<&str>,
    ) -> Result<AdminUser, RepositoryError> {
        debug!(slack_user_id = ?slack_user_id, "Updating admin user Slack user ID");
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            UPDATE admin.admin_user
            SET slack_user_id = $1
            WHERE id = $2
            RETURNING id, email, name, role as "role: AdminRole",
                      webauthn_user_id, slack_user_id, password_hash,
                      created_at as "created_at: DateTime<Utc>",
                      updated_at as "updated_at: DateTime<Utc>"
            "#,
            slack_user_id,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?
        .ok_or_else(|| {
            debug!("Admin user not found for Slack user ID update");
            RepositoryError::NotFound
        })?;

        let user: AdminUser = row.try_into()?;
        info!(user_id = %user.id.as_i32(), "Admin user Slack user ID updated");
        Ok(user)
    }

    /// Delete an admin user by their ID.
    ///
    /// This will cascade delete their credentials and sessions.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the user doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    #[instrument(skip(self), fields(id = %id.as_i32()), level = "debug")]
    pub async fn delete(&self, id: AdminUserId) -> Result<(), RepositoryError> {
        debug!("Deleting admin user");
        let result = sqlx::query!(
            r#"
            DELETE FROM admin.admin_user
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .execute(self.pool)
        .await?;

        if result.rows_affected() == 0 {
            debug!("Admin user not found for deletion");
            return Err(RepositoryError::NotFound);
        }

        info!(user_id = %id.as_i32(), "Admin user deleted");
        Ok(())
    }

    /// Count admin users by role.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(role = ?role), level = "debug")]
    pub async fn count_by_role(&self, role: AdminRole) -> Result<i64, RepositoryError> {
        debug!("Counting admin users by role");
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM admin.admin_user
            WHERE role = $1
            "#,
            role as AdminRole
        )
        .fetch_one(self.pool)
        .await?;

        debug!(count = count, "Admin user count by role retrieved");
        Ok(count)
    }

    // =========================================================================
    // Break-Glass Password Operations
    // =========================================================================

    /// Get an admin user by email, including their password hash for authentication.
    ///
    /// The password hash is returned separately and must NOT be stored
    /// beyond the authentication check. This method exists solely for
    /// the break-glass password authentication flow.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if the data is invalid.
    #[instrument(skip(self), fields(email = %email.as_str()), level = "debug")]
    pub async fn get_with_password_hash(
        &self,
        email: &Email,
    ) -> Result<Option<(AdminUser, Option<String>)>, RepositoryError> {
        debug!("Fetching admin user with password hash for authentication");
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            SELECT id, email, name, role as "role: AdminRole",
                   webauthn_user_id, slack_user_id, password_hash,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM admin.admin_user
            WHERE email = $1
            "#,
            email.as_str()
        )
        .fetch_optional(self.pool)
        .await?;

        if let Some(r) = row {
            let hash = r.password_hash.clone();
            let user: AdminUser = r.try_into()?;
            Ok(Some((user, hash)))
        } else {
            debug!("Admin user not found for password auth");
            Ok(None)
        }
    }

    /// Set or clear the password hash for an admin user.
    ///
    /// Pass `Some(hash)` to set a password, or `None` to remove it.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the user doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    #[instrument(skip(self, password_hash), fields(id = %id.as_i32()), level = "debug")]
    pub async fn set_password_hash(
        &self,
        id: AdminUserId,
        password_hash: Option<&str>,
    ) -> Result<(), RepositoryError> {
        debug!("Setting password hash for admin user");
        let result = sqlx::query!(
            r#"
            UPDATE admin.admin_user
            SET password_hash = $1
            WHERE id = $2
            "#,
            password_hash,
            id.as_i32()
        )
        .execute(self.pool)
        .await?;

        if result.rows_affected() == 0 {
            debug!("Admin user not found for password hash update");
            return Err(RepositoryError::NotFound);
        }

        info!(user_id = %id.as_i32(), "Admin user password hash updated");
        Ok(())
    }
}

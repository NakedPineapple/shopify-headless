//! Admin user repository for database operations.
//!
//! This module provides database access for admin users and their `WebAuthn` credentials.
//! All queries use sqlx macros for compile-time verification.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
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
    pub async fn list_all(&self) -> Result<Vec<AdminUser>, RepositoryError> {
        let rows = sqlx::query_as!(
            AdminUserRow,
            r#"
            SELECT id, email, name, role as "role: AdminRole",
                   webauthn_user_id,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM admin.admin_user
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(self.pool)
        .await?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    /// Get an admin user by their ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if the data is invalid.
    pub async fn get_by_id(&self, id: AdminUserId) -> Result<Option<AdminUser>, RepositoryError> {
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            SELECT id, email, name, role as "role: AdminRole",
                   webauthn_user_id,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM admin.admin_user
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?;

        row.map(TryInto::try_into).transpose()
    }

    /// Get an admin user by their email address.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if the data is invalid.
    pub async fn get_by_email(&self, email: &Email) -> Result<Option<AdminUser>, RepositoryError> {
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            SELECT id, email, name, role as "role: AdminRole",
                   webauthn_user_id,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM admin.admin_user
            WHERE email = $1
            "#,
            email.as_str()
        )
        .fetch_optional(self.pool)
        .await?;

        row.map(TryInto::try_into).transpose()
    }

    /// Get an admin user by their `WebAuthn` user ID (for discoverable credential authentication).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if the data is invalid.
    pub async fn get_by_webauthn_user_id(
        &self,
        webauthn_user_id: Uuid,
    ) -> Result<Option<AdminUser>, RepositoryError> {
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            SELECT id, email, name, role as "role: AdminRole",
                   webauthn_user_id,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM admin.admin_user
            WHERE webauthn_user_id = $1
            "#,
            webauthn_user_id
        )
        .fetch_optional(self.pool)
        .await?;

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
    pub async fn create(
        &self,
        email: &Email,
        name: &str,
        role: AdminRole,
        webauthn_user_id: Uuid,
    ) -> Result<AdminUser, RepositoryError> {
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            INSERT INTO admin.admin_user (email, name, role, webauthn_user_id)
            VALUES ($1, $2, $3, $4)
            RETURNING id, email, name, role as "role: AdminRole",
                      webauthn_user_id,
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
                return RepositoryError::Conflict("email already exists".to_owned());
            }
            RepositoryError::Database(e)
        })?;

        row.try_into()
    }

    /// Get all credentials for an admin user.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if any credential data is invalid.
    pub async fn get_credentials(
        &self,
        admin_user_id: AdminUserId,
    ) -> Result<Vec<AdminCredential>, RepositoryError> {
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
    pub async fn get_all_credentials(&self) -> Result<Vec<AdminCredential>, RepositoryError> {
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

        rows.into_iter().map(TryInto::try_into).collect()
    }

    /// Create a new credential for an admin user.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Conflict` if the credential ID already exists.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn create_credential(
        &self,
        admin_user_id: AdminUserId,
        passkey: &Passkey,
        name: &str,
    ) -> Result<AdminCredential, RepositoryError> {
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
                return RepositoryError::Conflict("credential already exists".to_owned());
            }
            RepositoryError::Database(e)
        })?;

        row.try_into()
    }

    /// Get a credential by its `WebAuthn` credential ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if the credential data is invalid.
    pub async fn get_credential_by_webauthn_id(
        &self,
        credential_id: &[u8],
    ) -> Result<Option<AdminCredential>, RepositoryError> {
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
    pub async fn update_credential(
        &self,
        credential_id: &[u8],
        passkey: &Passkey,
    ) -> Result<(), RepositoryError> {
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
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    /// Update an admin user's display name.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the user doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn update_name(
        &self,
        id: AdminUserId,
        name: &str,
    ) -> Result<AdminUser, RepositoryError> {
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            UPDATE admin.admin_user
            SET name = $1
            WHERE id = $2
            RETURNING id, email, name, role as "role: AdminRole",
                      webauthn_user_id,
                      created_at as "created_at: DateTime<Utc>",
                      updated_at as "updated_at: DateTime<Utc>"
            "#,
            name,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?
        .ok_or(RepositoryError::NotFound)?;

        row.try_into()
    }

    /// Update an admin user's email address.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the user doesn't exist.
    /// Returns `RepositoryError::Conflict` if the email is already used by another user.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn update_email(
        &self,
        id: AdminUserId,
        email: &Email,
    ) -> Result<AdminUser, RepositoryError> {
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            UPDATE admin.admin_user
            SET email = $1
            WHERE id = $2
            RETURNING id, email, name, role as "role: AdminRole",
                      webauthn_user_id,
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
                return RepositoryError::Conflict("email already exists".to_owned());
            }
            RepositoryError::Database(e)
        })?
        .ok_or(RepositoryError::NotFound)?;

        row.try_into()
    }

    /// Count the number of credentials for an admin user.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn count_credentials(
        &self,
        admin_user_id: AdminUserId,
    ) -> Result<i64, RepositoryError> {
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
    pub async fn delete_credential(
        &self,
        credential_id: AdminCredentialId,
        admin_user_id: AdminUserId,
    ) -> Result<(), RepositoryError> {
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
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    /// Update an admin user's role.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the user doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn update_role(
        &self,
        id: AdminUserId,
        role: AdminRole,
    ) -> Result<AdminUser, RepositoryError> {
        let row = sqlx::query_as!(
            AdminUserRow,
            r#"
            UPDATE admin.admin_user
            SET role = $1
            WHERE id = $2
            RETURNING id, email, name, role as "role: AdminRole",
                      webauthn_user_id,
                      created_at as "created_at: DateTime<Utc>",
                      updated_at as "updated_at: DateTime<Utc>"
            "#,
            role as AdminRole,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?
        .ok_or(RepositoryError::NotFound)?;

        row.try_into()
    }

    /// Delete an admin user by their ID.
    ///
    /// This will cascade delete their credentials and sessions.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the user doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn delete(&self, id: AdminUserId) -> Result<(), RepositoryError> {
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
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    /// Count admin users by role.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn count_by_role(&self, role: AdminRole) -> Result<i64, RepositoryError> {
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

        Ok(count)
    }
}

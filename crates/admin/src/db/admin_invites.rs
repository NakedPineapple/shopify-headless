//! Admin invite repository for database operations.
//!
//! Manages the invite allowlist for admin registration.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use naked_pineapple_core::{AdminUserId, Email};

use super::RepositoryError;
use crate::models::admin_user::AdminRole;

/// An admin invite record.
#[derive(Debug, Clone)]
pub struct AdminInvite {
    /// Unique identifier.
    pub id: i32,
    /// Email address that can register.
    pub email: Email,
    /// Display name for the new admin.
    pub name: String,
    /// Role to assign when the invite is used.
    pub role: AdminRole,
    /// Admin user who created this invite (None for CLI-created).
    pub invited_by: Option<AdminUserId>,
    /// When the invite was created.
    pub created_at: DateTime<Utc>,
    /// When the invite expires.
    pub expires_at: DateTime<Utc>,
    /// When the invite was used (None if unused).
    pub used_at: Option<DateTime<Utc>>,
    /// Admin user created when invite was used.
    pub used_by: Option<AdminUserId>,
}

impl AdminInvite {
    /// Returns true if this invite has already been used.
    #[must_use]
    pub const fn is_used(&self) -> bool {
        self.used_at.is_some()
    }

    /// Returns true if this invite has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Returns true if this invite can still be used.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.is_used() && !self.is_expired()
    }
}

/// Internal row type for database queries.
#[derive(Debug, sqlx::FromRow)]
struct AdminInviteRow {
    id: i32,
    email: String,
    name: String,
    role: AdminRole,
    invited_by: Option<i32>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    used_at: Option<DateTime<Utc>>,
    used_by: Option<i32>,
}

impl TryFrom<AdminInviteRow> for AdminInvite {
    type Error = RepositoryError;

    fn try_from(row: AdminInviteRow) -> Result<Self, Self::Error> {
        let email = Email::parse(&row.email).map_err(|e| {
            RepositoryError::DataCorruption(format!("invalid email in database: {e}"))
        })?;

        Ok(Self {
            id: row.id,
            email,
            name: row.name,
            role: row.role,
            invited_by: row.invited_by.map(AdminUserId::new),
            created_at: row.created_at,
            expires_at: row.expires_at,
            used_at: row.used_at,
            used_by: row.used_by.map(AdminUserId::new),
        })
    }
}

/// Repository for admin invite database operations.
pub struct AdminInviteRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> AdminInviteRepository<'a> {
    /// Create a new invite repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// List all invites (pending and used).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn list_all(&self) -> Result<Vec<AdminInvite>, RepositoryError> {
        let rows = sqlx::query_as!(
            AdminInviteRow,
            r#"
            SELECT id, email, name, role as "role: AdminRole",
                   invited_by, created_at as "created_at: DateTime<Utc>",
                   expires_at as "expires_at: DateTime<Utc>",
                   used_at as "used_at: DateTime<Utc>",
                   used_by
            FROM admin.admin_invite
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(self.pool)
        .await?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    /// Get an invite by email address.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get_by_email(&self, email: &str) -> Result<Option<AdminInvite>, RepositoryError> {
        let row = sqlx::query_as!(
            AdminInviteRow,
            r#"
            SELECT id, email, name, role as "role: AdminRole",
                   invited_by, created_at as "created_at: DateTime<Utc>",
                   expires_at as "expires_at: DateTime<Utc>",
                   used_at as "used_at: DateTime<Utc>",
                   used_by
            FROM admin.admin_invite
            WHERE email = $1
            "#,
            email
        )
        .fetch_optional(self.pool)
        .await?;

        row.map(TryInto::try_into).transpose()
    }

    /// Check if an email has a valid (unused, not expired) invite.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn is_valid_invite(&self, email: &str) -> Result<bool, RepositoryError> {
        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM admin.admin_invite
                WHERE email = $1
                  AND used_at IS NULL
                  AND expires_at > NOW()
            ) as "exists!"
            "#,
            email
        )
        .fetch_one(self.pool)
        .await?;

        Ok(exists)
    }

    /// Create a new invite.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Conflict` if an invite already exists for this email.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn create(
        &self,
        email: &str,
        name: &str,
        role: AdminRole,
        invited_by: Option<AdminUserId>,
        expires_in_days: i32,
    ) -> Result<AdminInvite, RepositoryError> {
        let invited_by_id = invited_by.map(|id| id.as_i32());

        let row = sqlx::query_as!(
            AdminInviteRow,
            r#"
            INSERT INTO admin.admin_invite (email, name, role, invited_by, expires_at)
            VALUES ($1, $2, $3, $4, NOW() + make_interval(days => $5))
            RETURNING id, email, name, role as "role: AdminRole",
                      invited_by, created_at as "created_at: DateTime<Utc>",
                      expires_at as "expires_at: DateTime<Utc>",
                      used_at as "used_at: DateTime<Utc>",
                      used_by
            "#,
            email,
            name,
            role as AdminRole,
            invited_by_id,
            expires_in_days
        )
        .fetch_one(self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e
                && db_err.is_unique_violation()
            {
                return RepositoryError::Conflict(
                    "invite already exists for this email".to_owned(),
                );
            }
            RepositoryError::Database(e)
        })?;

        row.try_into()
    }

    /// Mark an invite as used by a new admin user.
    ///
    /// This should be called within a transaction after creating the admin user.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the invite doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn mark_used(
        &self,
        email: &str,
        used_by: AdminUserId,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query!(
            r#"
            UPDATE admin.admin_invite
            SET used_at = NOW(), used_by = $1
            WHERE email = $2 AND used_at IS NULL
            "#,
            used_by.as_i32(),
            email
        )
        .execute(self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    /// Delete expired invites (cleanup).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn delete_expired(&self) -> Result<u64, RepositoryError> {
        let result = sqlx::query!(
            r#"
            DELETE FROM admin.admin_invite
            WHERE used_at IS NULL AND expires_at < NOW()
            "#
        )
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

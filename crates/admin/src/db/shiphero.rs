//! `ShipHero` credentials repository for database operations.
//!
//! This module provides database access for storing and retrieving
//! `ShipHero` JWT tokens obtained from email/password authentication.

use chrono::{DateTime, Utc};
use secrecy::SecretString;
use sqlx::PgPool;

use super::RepositoryError;

// =============================================================================
// Types
// =============================================================================

/// `ShipHero` credentials stored in the database.
///
/// Implements `Debug` manually to redact sensitive tokens.
#[derive(Clone)]
pub struct ShipHeroCredentials {
    /// Database ID.
    pub id: i32,
    /// Account name (default: "default").
    pub account_name: String,
    /// Email used for authentication (for display only).
    pub email: String,
    /// JWT access token (HIGH PRIVILEGE - redacted in debug output).
    pub access_token: SecretString,
    /// Refresh token (if provided by `ShipHero`).
    pub refresh_token: Option<SecretString>,
    /// Unix timestamp when access token expires.
    pub access_token_expires_at: i64,
    /// Unix timestamp when refresh token expires (if applicable).
    pub refresh_token_expires_at: Option<i64>,
    /// Admin user ID who connected the account.
    pub connected_by: Option<i32>,
    /// When the account was connected.
    pub connected_at: DateTime<Utc>,
    /// Last successful API call.
    pub last_used_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for ShipHeroCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShipHeroCredentials")
            .field("id", &self.id)
            .field("account_name", &self.account_name)
            .field("email", &self.email)
            .field("access_token", &"[REDACTED]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("access_token_expires_at", &self.access_token_expires_at)
            .field("refresh_token_expires_at", &self.refresh_token_expires_at)
            .field("connected_by", &self.connected_by)
            .field("connected_at", &self.connected_at)
            .field("last_used_at", &self.last_used_at)
            .finish()
    }
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct ShipHeroCredentialsRow {
    id: i32,
    account_name: String,
    email: String,
    access_token: String,
    refresh_token: Option<String>,
    access_token_expires_at: i64,
    refresh_token_expires_at: Option<i64>,
    connected_by: Option<i32>,
    connected_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
    #[allow(dead_code)]
    created_at: DateTime<Utc>,
    #[allow(dead_code)]
    updated_at: DateTime<Utc>,
}

impl From<ShipHeroCredentialsRow> for ShipHeroCredentials {
    fn from(row: ShipHeroCredentialsRow) -> Self {
        Self {
            id: row.id,
            account_name: row.account_name,
            email: row.email,
            access_token: SecretString::from(row.access_token),
            refresh_token: row.refresh_token.map(SecretString::from),
            access_token_expires_at: row.access_token_expires_at,
            refresh_token_expires_at: row.refresh_token_expires_at,
            connected_by: row.connected_by,
            connected_at: row.connected_at,
            last_used_at: row.last_used_at,
        }
    }
}

/// Parameters for saving `ShipHero` credentials.
#[derive(Debug)]
pub struct SaveCredentialsParams<'a> {
    /// Account name (use "default" for the default account).
    pub account_name: &'a str,
    /// Email used for authentication.
    pub email: &'a str,
    /// JWT access token.
    pub access_token: &'a str,
    /// Refresh token (if provided).
    pub refresh_token: Option<&'a str>,
    /// Unix timestamp when access token expires.
    pub access_token_expires_at: i64,
    /// Unix timestamp when refresh token expires.
    pub refresh_token_expires_at: Option<i64>,
    /// Admin user ID who connected the account.
    pub connected_by: Option<i32>,
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for `ShipHero` credentials database operations.
pub struct ShipHeroCredentialsRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> ShipHeroCredentialsRepository<'a> {
    /// Create a new `ShipHero` credentials repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Get credentials for an account.
    ///
    /// # Arguments
    ///
    /// * `account_name` - Account name (use "default" for the default account)
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get(
        &self,
        account_name: &str,
    ) -> Result<Option<ShipHeroCredentials>, RepositoryError> {
        let row = sqlx::query_as!(
            ShipHeroCredentialsRow,
            r#"
            SELECT
                id,
                account_name,
                email,
                access_token,
                refresh_token,
                access_token_expires_at,
                refresh_token_expires_at,
                connected_by,
                connected_at as "connected_at: DateTime<Utc>",
                last_used_at as "last_used_at: DateTime<Utc>",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.shiphero_credentials
            WHERE account_name = $1
            "#,
            account_name
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(ShipHeroCredentials::from))
    }

    /// Get the default account credentials.
    ///
    /// Convenience method for `get("default")`.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get_default(&self) -> Result<Option<ShipHeroCredentials>, RepositoryError> {
        self.get("default").await
    }

    /// Save or update credentials for an account.
    ///
    /// Uses upsert to handle both new and existing credentials.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn save(&self, params: &SaveCredentialsParams<'_>) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"
            INSERT INTO admin.shiphero_credentials (
                account_name,
                email,
                access_token,
                refresh_token,
                access_token_expires_at,
                refresh_token_expires_at,
                connected_by,
                connected_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, (CURRENT_TIMESTAMP AT TIME ZONE 'utc'))
            ON CONFLICT(account_name) DO UPDATE SET
                email = EXCLUDED.email,
                access_token = EXCLUDED.access_token,
                refresh_token = EXCLUDED.refresh_token,
                access_token_expires_at = EXCLUDED.access_token_expires_at,
                refresh_token_expires_at = EXCLUDED.refresh_token_expires_at,
                connected_by = EXCLUDED.connected_by,
                connected_at = EXCLUDED.connected_at,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            "#,
            params.account_name,
            params.email,
            params.access_token,
            params.refresh_token,
            params.access_token_expires_at,
            params.refresh_token_expires_at,
            params.connected_by
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Update only the tokens (after a refresh).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn update_tokens(
        &self,
        account_name: &str,
        access_token: &str,
        refresh_token: Option<&str>,
        access_token_expires_at: i64,
        refresh_token_expires_at: Option<i64>,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            r#"
            UPDATE admin.shiphero_credentials
            SET
                access_token = $2,
                refresh_token = $3,
                access_token_expires_at = $4,
                refresh_token_expires_at = $5,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            WHERE account_name = $1
            "#,
            account_name,
            access_token,
            refresh_token,
            access_token_expires_at,
            refresh_token_expires_at
        )
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update the `last_used_at` timestamp.
    ///
    /// Call this after a successful API call to track usage.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn touch(&self, account_name: &str) -> Result<(), RepositoryError> {
        sqlx::query!(
            r"
            UPDATE admin.shiphero_credentials
            SET last_used_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            WHERE account_name = $1
            ",
            account_name
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Delete credentials for an account.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn delete(&self, account_name: &str) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            r"
            DELETE FROM admin.shiphero_credentials
            WHERE account_name = $1
            ",
            account_name
        )
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Check if credentials exist for an account.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn exists(&self, account_name: &str) -> Result<bool, RepositoryError> {
        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM admin.shiphero_credentials WHERE account_name = $1
            ) as "exists!"
            "#,
            account_name
        )
        .fetch_one(self.pool)
        .await?;

        Ok(exists)
    }
}

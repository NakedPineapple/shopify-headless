//! Pinterest credentials repository for database operations.
//!
//! This module provides database access for storing and retrieving
//! Pinterest API credentials (OAuth 2.0 + Ad Account ID).

use chrono::{DateTime, Utc};
use secrecy::SecretString;
use sqlx::PgPool;
use tracing::{debug, info, instrument};

use super::RepositoryError;

// =============================================================================
// Types
// =============================================================================

/// Pinterest credentials stored in the database.
///
/// Implements `Debug` manually to redact sensitive tokens.
#[derive(Clone)]
pub struct PinterestCredentials {
    /// Database ID.
    pub id: i32,
    /// Account name (default: "default").
    pub account_name: String,
    /// Pinterest App ID.
    pub app_id: String,
    /// Pinterest App Secret (HIGH PRIVILEGE - redacted in debug output).
    pub app_secret: SecretString,
    /// OAuth access token (HIGH PRIVILEGE - redacted in debug output).
    pub access_token: SecretString,
    /// OAuth refresh token (HIGH PRIVILEGE - redacted in debug output).
    pub refresh_token: SecretString,
    /// Ad Account ID (required for Conversions API).
    pub ad_account_id: String,
    /// Unix timestamp when token expires.
    pub token_expires_at: Option<i64>,
    /// Admin user ID who connected the account.
    pub connected_by: Option<i32>,
    /// When the account was connected.
    pub connected_at: DateTime<Utc>,
    /// Last successful API call.
    pub last_used_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for PinterestCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PinterestCredentials")
            .field("id", &self.id)
            .field("account_name", &self.account_name)
            .field("app_id", &self.app_id)
            .field("app_secret", &"[REDACTED]")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("ad_account_id", &self.ad_account_id)
            .field("token_expires_at", &self.token_expires_at)
            .field("connected_by", &self.connected_by)
            .field("connected_at", &self.connected_at)
            .field("last_used_at", &self.last_used_at)
            .finish()
    }
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct PinterestCredentialsRow {
    id: i32,
    account_name: String,
    app_id: String,
    app_secret: String,
    access_token: String,
    refresh_token: String,
    ad_account_id: String,
    token_expires_at: Option<i64>,
    connected_by: Option<i32>,
    connected_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
}

impl From<PinterestCredentialsRow> for PinterestCredentials {
    fn from(row: PinterestCredentialsRow) -> Self {
        Self {
            id: row.id,
            account_name: row.account_name,
            app_id: row.app_id,
            app_secret: SecretString::from(row.app_secret),
            access_token: SecretString::from(row.access_token),
            refresh_token: SecretString::from(row.refresh_token),
            ad_account_id: row.ad_account_id,
            token_expires_at: row.token_expires_at,
            connected_by: row.connected_by,
            connected_at: row.connected_at,
            last_used_at: row.last_used_at,
        }
    }
}

/// Parameters for saving Pinterest credentials.
pub struct SavePinterestParams<'a> {
    /// Account name (use "default" for the default account).
    pub account_name: &'a str,
    /// Pinterest App ID.
    pub app_id: &'a str,
    /// Pinterest App Secret.
    pub app_secret: &'a str,
    /// OAuth access token.
    pub access_token: &'a str,
    /// OAuth refresh token.
    pub refresh_token: &'a str,
    /// Ad Account ID.
    pub ad_account_id: &'a str,
    /// Admin user ID who connected the account.
    pub connected_by: Option<i32>,
}

impl std::fmt::Debug for SavePinterestParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SavePinterestParams")
            .field("account_name", &self.account_name)
            .field("app_id", &self.app_id)
            .field("app_secret", &"[REDACTED]")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("ad_account_id", &self.ad_account_id)
            .field("connected_by", &self.connected_by)
            .finish()
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for Pinterest credentials database operations.
pub struct PinterestCredentialsRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> PinterestCredentialsRepository<'a> {
    /// Create a new Pinterest credentials repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Get the default account credentials.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_default(&self) -> Result<Option<PinterestCredentials>, RepositoryError> {
        debug!("Fetching default Pinterest credentials");

        let row = sqlx::query_as!(
            PinterestCredentialsRow,
            r#"
            SELECT
                id,
                account_name,
                app_id,
                app_secret,
                access_token,
                refresh_token,
                ad_account_id,
                token_expires_at,
                connected_by,
                connected_at as "connected_at: DateTime<Utc>",
                last_used_at as "last_used_at: DateTime<Utc>"
            FROM admin.pinterest_credentials
            WHERE account_name = 'default'
            "#
        )
        .fetch_optional(self.pool)
        .await?;

        if row.is_none() {
            debug!("Pinterest credentials not found");
        }

        Ok(row.map(PinterestCredentials::from))
    }

    /// Save or update credentials for an account.
    ///
    /// Uses upsert to handle both new and existing credentials.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(account_name = %params.account_name), level = "debug")]
    pub async fn save(&self, params: &SavePinterestParams<'_>) -> Result<(), RepositoryError> {
        debug!("Saving Pinterest credentials");

        sqlx::query!(
            r#"
            INSERT INTO admin.pinterest_credentials (
                account_name,
                app_id,
                app_secret,
                access_token,
                refresh_token,
                ad_account_id,
                connected_by,
                connected_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, (CURRENT_TIMESTAMP AT TIME ZONE 'utc'))
            ON CONFLICT(account_name) DO UPDATE SET
                app_id = EXCLUDED.app_id,
                app_secret = EXCLUDED.app_secret,
                access_token = EXCLUDED.access_token,
                refresh_token = EXCLUDED.refresh_token,
                ad_account_id = EXCLUDED.ad_account_id,
                connected_by = EXCLUDED.connected_by,
                connected_at = EXCLUDED.connected_at,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            "#,
            params.account_name,
            params.app_id,
            params.app_secret,
            params.access_token,
            params.refresh_token,
            params.ad_account_id,
            params.connected_by
        )
        .execute(self.pool)
        .await?;

        info!("Pinterest credentials saved");

        Ok(())
    }

    /// Update the cached token after refresh.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, access_token), fields(account_name = %account_name), level = "debug")]
    pub async fn update_token(
        &self,
        account_name: &str,
        access_token: &str,
        expires_at: i64,
    ) -> Result<bool, RepositoryError> {
        debug!("Updating Pinterest access token");

        let result = sqlx::query!(
            r#"
            UPDATE admin.pinterest_credentials
            SET
                access_token = $2,
                token_expires_at = $3,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            WHERE account_name = $1
            "#,
            account_name,
            access_token,
            expires_at
        )
        .execute(self.pool)
        .await?;

        let updated = result.rows_affected() > 0;
        if updated {
            info!("Pinterest access token updated");
        } else {
            debug!("Pinterest credentials not found for token update");
        }

        Ok(updated)
    }

    /// Update the `last_used_at` timestamp.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(account_name = %account_name), level = "debug")]
    pub async fn touch(&self, account_name: &str) -> Result<(), RepositoryError> {
        debug!("Updating Pinterest last_used_at timestamp");

        sqlx::query!(
            r"
            UPDATE admin.pinterest_credentials
            SET last_used_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            WHERE account_name = $1
            ",
            account_name
        )
        .execute(self.pool)
        .await?;

        debug!("Pinterest last_used_at timestamp updated");

        Ok(())
    }

    /// Delete credentials for an account.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(account_name = %account_name), level = "debug")]
    pub async fn delete(&self, account_name: &str) -> Result<bool, RepositoryError> {
        debug!("Deleting Pinterest credentials");

        let result = sqlx::query!(
            r"
            DELETE FROM admin.pinterest_credentials
            WHERE account_name = $1
            ",
            account_name
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!("Pinterest credentials deleted");
        } else {
            debug!("Pinterest credentials not found for deletion");
        }

        Ok(deleted)
    }

    /// Check if credentials exist for an account.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(account_name = %account_name), level = "debug")]
    pub async fn exists(&self, account_name: &str) -> Result<bool, RepositoryError> {
        debug!("Checking if Pinterest credentials exist");

        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM admin.pinterest_credentials WHERE account_name = $1
            ) as "exists!"
            "#,
            account_name
        )
        .fetch_one(self.pool)
        .await?;

        debug!(exists = %exists, "Pinterest credentials existence check complete");

        Ok(exists)
    }
}

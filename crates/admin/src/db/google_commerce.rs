//! Google Merchant Center credentials repository for database operations.
//!
//! This module provides database access for storing and retrieving
//! Google Merchant Center API credentials (OAuth 2.0 + Merchant ID).

use chrono::{DateTime, Utc};
use secrecy::SecretString;
use sqlx::PgPool;
use tracing::{debug, info, instrument};

use super::RepositoryError;

// =============================================================================
// Types
// =============================================================================

/// Google Merchant Center credentials stored in the database.
///
/// Implements `Debug` manually to redact sensitive tokens.
#[derive(Clone)]
pub struct GoogleCredentials {
    /// Database ID.
    pub id: i32,
    /// Account name (default: "default").
    pub account_name: String,
    /// Google Merchant Center ID.
    pub merchant_id: String,
    /// OAuth 2.0 Client ID.
    pub client_id: String,
    /// OAuth 2.0 Client Secret (HIGH PRIVILEGE - redacted in debug output).
    pub client_secret: SecretString,
    /// OAuth access token (HIGH PRIVILEGE - redacted in debug output).
    pub access_token: SecretString,
    /// OAuth refresh token (HIGH PRIVILEGE - redacted in debug output).
    pub refresh_token: SecretString,
    /// Unix timestamp when token expires.
    pub token_expires_at: Option<i64>,
    /// Admin user ID who connected the account.
    pub connected_by: Option<i32>,
    /// When the account was connected.
    pub connected_at: DateTime<Utc>,
    /// Last successful API call.
    pub last_used_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for GoogleCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleCredentials")
            .field("id", &self.id)
            .field("account_name", &self.account_name)
            .field("merchant_id", &self.merchant_id)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("token_expires_at", &self.token_expires_at)
            .field("connected_by", &self.connected_by)
            .field("connected_at", &self.connected_at)
            .field("last_used_at", &self.last_used_at)
            .finish()
    }
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct GoogleCredentialsRow {
    id: i32,
    account_name: String,
    merchant_id: String,
    client_id: String,
    client_secret: String,
    access_token: String,
    refresh_token: String,
    token_expires_at: Option<i64>,
    connected_by: Option<i32>,
    connected_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
}

impl From<GoogleCredentialsRow> for GoogleCredentials {
    fn from(row: GoogleCredentialsRow) -> Self {
        Self {
            id: row.id,
            account_name: row.account_name,
            merchant_id: row.merchant_id,
            client_id: row.client_id,
            client_secret: SecretString::from(row.client_secret),
            access_token: SecretString::from(row.access_token),
            refresh_token: SecretString::from(row.refresh_token),
            token_expires_at: row.token_expires_at,
            connected_by: row.connected_by,
            connected_at: row.connected_at,
            last_used_at: row.last_used_at,
        }
    }
}

/// Parameters for saving Google Merchant Center credentials.
pub struct SaveGoogleParams<'a> {
    /// Account name (use "default" for the default account).
    pub account_name: &'a str,
    /// Google Merchant Center ID.
    pub merchant_id: &'a str,
    /// OAuth 2.0 Client ID.
    pub client_id: &'a str,
    /// OAuth 2.0 Client Secret.
    pub client_secret: &'a str,
    /// OAuth access token.
    pub access_token: &'a str,
    /// OAuth refresh token.
    pub refresh_token: &'a str,
    /// Admin user ID who connected the account.
    pub connected_by: Option<i32>,
}

impl std::fmt::Debug for SaveGoogleParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SaveGoogleParams")
            .field("account_name", &self.account_name)
            .field("merchant_id", &self.merchant_id)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("connected_by", &self.connected_by)
            .finish()
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for Google Merchant Center credentials database operations.
pub struct GoogleCredentialsRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> GoogleCredentialsRepository<'a> {
    /// Create a new Google credentials repository.
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
    pub async fn get_default(&self) -> Result<Option<GoogleCredentials>, RepositoryError> {
        debug!("Fetching default Google credentials");

        let row = sqlx::query_as!(
            GoogleCredentialsRow,
            r#"
            SELECT
                id,
                account_name,
                merchant_id,
                client_id,
                client_secret,
                access_token,
                refresh_token,
                token_expires_at,
                connected_by,
                connected_at as "connected_at: DateTime<Utc>",
                last_used_at as "last_used_at: DateTime<Utc>"
            FROM admin.google_credentials
            WHERE account_name = 'default'
            "#
        )
        .fetch_optional(self.pool)
        .await?;

        if row.is_none() {
            debug!("Google credentials not found");
        }

        Ok(row.map(GoogleCredentials::from))
    }

    /// Save or update credentials for an account.
    ///
    /// Uses upsert to handle both new and existing credentials.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(account_name = %params.account_name), level = "debug")]
    pub async fn save(&self, params: &SaveGoogleParams<'_>) -> Result<(), RepositoryError> {
        debug!("Saving Google credentials");

        sqlx::query!(
            r#"
            INSERT INTO admin.google_credentials (
                account_name,
                merchant_id,
                client_id,
                client_secret,
                access_token,
                refresh_token,
                connected_by,
                connected_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, (CURRENT_TIMESTAMP AT TIME ZONE 'utc'))
            ON CONFLICT(account_name) DO UPDATE SET
                merchant_id = EXCLUDED.merchant_id,
                client_id = EXCLUDED.client_id,
                client_secret = EXCLUDED.client_secret,
                access_token = EXCLUDED.access_token,
                refresh_token = EXCLUDED.refresh_token,
                connected_by = EXCLUDED.connected_by,
                connected_at = EXCLUDED.connected_at,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            "#,
            params.account_name,
            params.merchant_id,
            params.client_id,
            params.client_secret,
            params.access_token,
            params.refresh_token,
            params.connected_by
        )
        .execute(self.pool)
        .await?;

        info!("Google credentials saved");

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
        debug!("Updating Google access token");

        let result = sqlx::query!(
            r#"
            UPDATE admin.google_credentials
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
            info!("Google access token updated");
        } else {
            debug!("Google credentials not found for token update");
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
        debug!("Updating Google last_used_at timestamp");

        sqlx::query!(
            r"
            UPDATE admin.google_credentials
            SET last_used_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            WHERE account_name = $1
            ",
            account_name
        )
        .execute(self.pool)
        .await?;

        debug!("Google last_used_at timestamp updated");

        Ok(())
    }

    /// Delete credentials for an account.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(account_name = %account_name), level = "debug")]
    pub async fn delete(&self, account_name: &str) -> Result<bool, RepositoryError> {
        debug!("Deleting Google credentials");

        let result = sqlx::query!(
            r"
            DELETE FROM admin.google_credentials
            WHERE account_name = $1
            ",
            account_name
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!("Google credentials deleted");
        } else {
            debug!("Google credentials not found for deletion");
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
        debug!("Checking if Google credentials exist");

        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM admin.google_credentials WHERE account_name = $1
            ) as "exists!"
            "#,
            account_name
        )
        .fetch_one(self.pool)
        .await?;

        debug!(exists = %exists, "Google credentials existence check complete");

        Ok(exists)
    }
}

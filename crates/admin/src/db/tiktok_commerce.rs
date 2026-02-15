//! TikTok Shop credentials repository for database operations.
//!
//! This module provides database access for storing and retrieving
//! TikTok Shop API credentials (App Key, App Secret, and OAuth tokens).

use chrono::{DateTime, Utc};
use secrecy::SecretString;
use sqlx::PgPool;
use tracing::{debug, info, instrument};

use super::RepositoryError;

// =============================================================================
// Types
// =============================================================================

/// TikTok Shop credentials stored in the database.
///
/// Implements `Debug` manually to redact sensitive tokens.
#[derive(Clone)]
pub struct TikTokShopCredentials {
    /// Database ID.
    pub id: i32,
    /// Account name (default: "default").
    pub account_name: String,
    /// TikTok App Key.
    pub app_key: String,
    /// TikTok App Secret (HIGH PRIVILEGE - redacted in debug output).
    pub app_secret: SecretString,
    /// OAuth Access Token (HIGH PRIVILEGE - redacted in debug output).
    pub access_token: SecretString,
    /// OAuth Refresh Token (HIGH PRIVILEGE - redacted in debug output).
    pub refresh_token: SecretString,
    /// TikTok Shop ID.
    pub shop_id: String,
    /// TikTok Shop Cipher.
    pub shop_cipher: String,
    /// Unix timestamp when token expires.
    pub token_expires_at: Option<i64>,
    /// Admin user ID who connected the account.
    pub connected_by: Option<i32>,
    /// When the account was connected.
    pub connected_at: DateTime<Utc>,
    /// Last successful API call.
    pub last_used_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for TikTokShopCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TikTokShopCredentials")
            .field("id", &self.id)
            .field("account_name", &self.account_name)
            .field("app_key", &self.app_key)
            .field("app_secret", &"[REDACTED]")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("shop_id", &self.shop_id)
            .field("shop_cipher", &self.shop_cipher)
            .field("token_expires_at", &self.token_expires_at)
            .field("connected_by", &self.connected_by)
            .field("connected_at", &self.connected_at)
            .field("last_used_at", &self.last_used_at)
            .finish()
    }
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct TikTokShopCredentialsRow {
    id: i32,
    account_name: String,
    app_key: String,
    app_secret: String,
    access_token: String,
    refresh_token: String,
    shop_id: String,
    shop_cipher: String,
    token_expires_at: Option<i64>,
    connected_by: Option<i32>,
    connected_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
}

impl From<TikTokShopCredentialsRow> for TikTokShopCredentials {
    fn from(row: TikTokShopCredentialsRow) -> Self {
        Self {
            id: row.id,
            account_name: row.account_name,
            app_key: row.app_key,
            app_secret: SecretString::from(row.app_secret),
            access_token: SecretString::from(row.access_token),
            refresh_token: SecretString::from(row.refresh_token),
            shop_id: row.shop_id,
            shop_cipher: row.shop_cipher,
            token_expires_at: row.token_expires_at,
            connected_by: row.connected_by,
            connected_at: row.connected_at,
            last_used_at: row.last_used_at,
        }
    }
}

/// Parameters for saving TikTok Shop credentials.
pub struct SaveTikTokShopParams<'a> {
    /// Account name (use "default" for the default account).
    pub account_name: &'a str,
    /// TikTok App Key.
    pub app_key: &'a str,
    /// TikTok App Secret.
    pub app_secret: &'a str,
    /// OAuth Access Token.
    pub access_token: &'a str,
    /// OAuth Refresh Token.
    pub refresh_token: &'a str,
    /// TikTok Shop ID.
    pub shop_id: &'a str,
    /// TikTok Shop Cipher.
    pub shop_cipher: &'a str,
    /// Admin user ID who connected the account.
    pub connected_by: Option<i32>,
}

impl std::fmt::Debug for SaveTikTokShopParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SaveTikTokShopParams")
            .field("account_name", &self.account_name)
            .field("app_key", &self.app_key)
            .field("app_secret", &"[REDACTED]")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("shop_id", &self.shop_id)
            .field("shop_cipher", &self.shop_cipher)
            .field("connected_by", &self.connected_by)
            .finish()
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for TikTok Shop credentials database operations.
pub struct TikTokShopCredentialsRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> TikTokShopCredentialsRepository<'a> {
    /// Create a new TikTok Shop credentials repository.
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
    pub async fn get_default(&self) -> Result<Option<TikTokShopCredentials>, RepositoryError> {
        debug!("Fetching default TikTok Shop credentials");

        let row = sqlx::query_as!(
            TikTokShopCredentialsRow,
            r#"
            SELECT
                id,
                account_name,
                app_key,
                app_secret,
                access_token,
                refresh_token,
                shop_id,
                shop_cipher,
                token_expires_at,
                connected_by,
                connected_at as "connected_at: DateTime<Utc>",
                last_used_at as "last_used_at: DateTime<Utc>"
            FROM admin.tiktok_shop_credentials
            WHERE account_name = 'default'
            "#
        )
        .fetch_optional(self.pool)
        .await?;

        if row.is_none() {
            debug!("TikTok Shop credentials not found");
        }

        Ok(row.map(TikTokShopCredentials::from))
    }

    /// Save or update credentials for an account.
    ///
    /// Uses upsert to handle both new and existing credentials.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(account_name = %params.account_name), level = "debug")]
    pub async fn save(&self, params: &SaveTikTokShopParams<'_>) -> Result<(), RepositoryError> {
        debug!("Saving TikTok Shop credentials");

        sqlx::query!(
            r#"
            INSERT INTO admin.tiktok_shop_credentials (
                account_name,
                app_key,
                app_secret,
                access_token,
                refresh_token,
                shop_id,
                shop_cipher,
                connected_by,
                connected_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, (CURRENT_TIMESTAMP AT TIME ZONE 'utc'))
            ON CONFLICT(account_name) DO UPDATE SET
                app_key = EXCLUDED.app_key,
                app_secret = EXCLUDED.app_secret,
                access_token = EXCLUDED.access_token,
                refresh_token = EXCLUDED.refresh_token,
                shop_id = EXCLUDED.shop_id,
                shop_cipher = EXCLUDED.shop_cipher,
                connected_by = EXCLUDED.connected_by,
                connected_at = EXCLUDED.connected_at,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            "#,
            params.account_name,
            params.app_key,
            params.app_secret,
            params.access_token,
            params.refresh_token,
            params.shop_id,
            params.shop_cipher,
            params.connected_by
        )
        .execute(self.pool)
        .await?;

        info!("TikTok Shop credentials saved");

        Ok(())
    }

    /// Update the access token and expiry.
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
        debug!("Updating TikTok Shop access token");

        let result = sqlx::query!(
            r#"
            UPDATE admin.tiktok_shop_credentials
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
            info!("TikTok Shop access token updated");
        } else {
            debug!("TikTok Shop credentials not found for token update");
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
        debug!("Updating TikTok Shop last_used_at timestamp");

        sqlx::query!(
            r"
            UPDATE admin.tiktok_shop_credentials
            SET last_used_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            WHERE account_name = $1
            ",
            account_name
        )
        .execute(self.pool)
        .await?;

        debug!("TikTok Shop last_used_at timestamp updated");

        Ok(())
    }

    /// Delete credentials for an account.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(account_name = %account_name), level = "debug")]
    pub async fn delete(&self, account_name: &str) -> Result<bool, RepositoryError> {
        debug!("Deleting TikTok Shop credentials");

        let result = sqlx::query!(
            r"
            DELETE FROM admin.tiktok_shop_credentials
            WHERE account_name = $1
            ",
            account_name
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!("TikTok Shop credentials deleted");
        } else {
            debug!("TikTok Shop credentials not found for deletion");
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
        debug!("Checking if TikTok Shop credentials exist");

        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM admin.tiktok_shop_credentials WHERE account_name = $1
            ) as "exists!"
            "#,
            account_name
        )
        .fetch_one(self.pool)
        .await?;

        debug!(exists = %exists, "TikTok Shop credentials existence check complete");

        Ok(exists)
    }
}

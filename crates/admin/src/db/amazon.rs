//! Amazon SP-API credentials repository for database operations.
//!
//! This module provides database access for storing and retrieving
//! Amazon SP-API credentials (LWA OAuth tokens + AWS IAM keys).

use chrono::{DateTime, Utc};
use secrecy::SecretString;
use sqlx::PgPool;
use tracing::{debug, info, instrument};

use super::RepositoryError;

// =============================================================================
// Types
// =============================================================================

/// Amazon SP-API credentials stored in the database.
///
/// Implements `Debug` manually to redact sensitive tokens and keys.
#[derive(Clone)]
pub struct AmazonSpCredentials {
    /// Database ID.
    pub id: i32,
    /// Account name (default: "default").
    pub account_name: String,
    /// LWA client ID.
    pub lwa_client_id: String,
    /// LWA client secret (HIGH PRIVILEGE - redacted in debug output).
    pub lwa_client_secret: SecretString,
    /// LWA refresh token (HIGH PRIVILEGE - redacted in debug output).
    pub lwa_refresh_token: SecretString,
    /// AWS IAM access key ID.
    pub aws_access_key_id: String,
    /// AWS IAM secret access key (HIGH PRIVILEGE - redacted in debug output).
    pub aws_secret_access_key: SecretString,
    /// Amazon seller ID.
    pub seller_id: String,
    /// Amazon marketplace ID (e.g., ATVPDKIKX0DER for US).
    pub marketplace_id: String,
    /// Cached LWA access token.
    pub access_token: Option<SecretString>,
    /// Unix timestamp when access token expires.
    pub access_token_expires_at: Option<i64>,
    /// Admin user ID who connected the account.
    pub connected_by: Option<i32>,
    /// When the account was connected.
    pub connected_at: DateTime<Utc>,
    /// Last successful API call.
    pub last_used_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for AmazonSpCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AmazonSpCredentials")
            .field("id", &self.id)
            .field("account_name", &self.account_name)
            .field("lwa_client_id", &self.lwa_client_id)
            .field("lwa_client_secret", &"[REDACTED]")
            .field("lwa_refresh_token", &"[REDACTED]")
            .field("aws_access_key_id", &self.aws_access_key_id)
            .field("aws_secret_access_key", &"[REDACTED]")
            .field("seller_id", &self.seller_id)
            .field("marketplace_id", &self.marketplace_id)
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("access_token_expires_at", &self.access_token_expires_at)
            .field("connected_by", &self.connected_by)
            .field("connected_at", &self.connected_at)
            .field("last_used_at", &self.last_used_at)
            .finish()
    }
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct AmazonSpCredentialsRow {
    id: i32,
    account_name: String,
    lwa_client_id: String,
    lwa_client_secret: String,
    lwa_refresh_token: String,
    aws_access_key_id: String,
    aws_secret_access_key: String,
    seller_id: String,
    marketplace_id: String,
    access_token: Option<String>,
    access_token_expires_at: Option<i64>,
    connected_by: Option<i32>,
    connected_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
}

impl From<AmazonSpCredentialsRow> for AmazonSpCredentials {
    fn from(row: AmazonSpCredentialsRow) -> Self {
        Self {
            id: row.id,
            account_name: row.account_name,
            lwa_client_id: row.lwa_client_id,
            lwa_client_secret: SecretString::from(row.lwa_client_secret),
            lwa_refresh_token: SecretString::from(row.lwa_refresh_token),
            aws_access_key_id: row.aws_access_key_id,
            aws_secret_access_key: SecretString::from(row.aws_secret_access_key),
            seller_id: row.seller_id,
            marketplace_id: row.marketplace_id,
            access_token: row.access_token.map(SecretString::from),
            access_token_expires_at: row.access_token_expires_at,
            connected_by: row.connected_by,
            connected_at: row.connected_at,
            last_used_at: row.last_used_at,
        }
    }
}

/// Parameters for saving Amazon SP-API credentials.
pub struct SaveAmazonSpParams<'a> {
    /// Account name (use "default" for the default account).
    pub account_name: &'a str,
    /// LWA client ID.
    pub lwa_client_id: &'a str,
    /// LWA client secret.
    pub lwa_client_secret: &'a str,
    /// LWA refresh token.
    pub lwa_refresh_token: &'a str,
    /// AWS IAM access key ID.
    pub aws_access_key_id: &'a str,
    /// AWS IAM secret access key.
    pub aws_secret_access_key: &'a str,
    /// Amazon seller ID.
    pub seller_id: &'a str,
    /// Admin user ID who connected the account.
    pub connected_by: Option<i32>,
}

impl std::fmt::Debug for SaveAmazonSpParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SaveAmazonSpParams")
            .field("account_name", &self.account_name)
            .field("lwa_client_id", &self.lwa_client_id)
            .field("lwa_client_secret", &"[REDACTED]")
            .field("lwa_refresh_token", &"[REDACTED]")
            .field("aws_access_key_id", &self.aws_access_key_id)
            .field("aws_secret_access_key", &"[REDACTED]")
            .field("seller_id", &self.seller_id)
            .field("connected_by", &self.connected_by)
            .finish()
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for Amazon SP-API credentials database operations.
pub struct AmazonSpCredentialsRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> AmazonSpCredentialsRepository<'a> {
    /// Create a new Amazon SP-API credentials repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Get credentials for an account.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(account_name = %account_name), level = "debug")]
    pub async fn get(
        &self,
        account_name: &str,
    ) -> Result<Option<AmazonSpCredentials>, RepositoryError> {
        debug!("Fetching Amazon SP-API credentials");

        let row = sqlx::query_as!(
            AmazonSpCredentialsRow,
            r#"
            SELECT
                id,
                account_name,
                lwa_client_id,
                lwa_client_secret,
                lwa_refresh_token,
                aws_access_key_id,
                aws_secret_access_key,
                seller_id,
                marketplace_id,
                access_token,
                access_token_expires_at,
                connected_by,
                connected_at as "connected_at: DateTime<Utc>",
                last_used_at as "last_used_at: DateTime<Utc>"
            FROM admin.amazon_sp_credentials
            WHERE account_name = $1
            "#,
            account_name
        )
        .fetch_optional(self.pool)
        .await?;

        if row.is_none() {
            debug!("Amazon SP-API credentials not found");
        }

        Ok(row.map(AmazonSpCredentials::from))
    }

    /// Get the default account credentials.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_default(&self) -> Result<Option<AmazonSpCredentials>, RepositoryError> {
        debug!("Fetching default Amazon SP-API credentials");
        self.get("default").await
    }

    /// Save or update credentials for an account.
    ///
    /// Uses upsert to handle both new and existing credentials.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(account_name = %params.account_name), level = "debug")]
    pub async fn save(&self, params: &SaveAmazonSpParams<'_>) -> Result<(), RepositoryError> {
        debug!("Saving Amazon SP-API credentials");

        sqlx::query!(
            r#"
            INSERT INTO admin.amazon_sp_credentials (
                account_name,
                lwa_client_id,
                lwa_client_secret,
                lwa_refresh_token,
                aws_access_key_id,
                aws_secret_access_key,
                seller_id,
                connected_by,
                connected_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, (CURRENT_TIMESTAMP AT TIME ZONE 'utc'))
            ON CONFLICT(account_name) DO UPDATE SET
                lwa_client_id = EXCLUDED.lwa_client_id,
                lwa_client_secret = EXCLUDED.lwa_client_secret,
                lwa_refresh_token = EXCLUDED.lwa_refresh_token,
                aws_access_key_id = EXCLUDED.aws_access_key_id,
                aws_secret_access_key = EXCLUDED.aws_secret_access_key,
                seller_id = EXCLUDED.seller_id,
                connected_by = EXCLUDED.connected_by,
                connected_at = EXCLUDED.connected_at,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            "#,
            params.account_name,
            params.lwa_client_id,
            params.lwa_client_secret,
            params.lwa_refresh_token,
            params.aws_access_key_id,
            params.aws_secret_access_key,
            params.seller_id,
            params.connected_by
        )
        .execute(self.pool)
        .await?;

        info!("Amazon SP-API credentials saved");

        Ok(())
    }

    /// Update the cached access token.
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
        debug!("Updating Amazon SP-API access token");

        let result = sqlx::query!(
            r#"
            UPDATE admin.amazon_sp_credentials
            SET
                access_token = $2,
                access_token_expires_at = $3,
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
            info!("Amazon SP-API access token updated");
        } else {
            debug!("Amazon SP-API credentials not found for token update");
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
        debug!("Updating Amazon SP-API last_used_at timestamp");

        sqlx::query!(
            r"
            UPDATE admin.amazon_sp_credentials
            SET last_used_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            WHERE account_name = $1
            ",
            account_name
        )
        .execute(self.pool)
        .await?;

        debug!("Amazon SP-API last_used_at timestamp updated");

        Ok(())
    }

    /// Delete credentials for an account.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(account_name = %account_name), level = "debug")]
    pub async fn delete(&self, account_name: &str) -> Result<bool, RepositoryError> {
        debug!("Deleting Amazon SP-API credentials");

        let result = sqlx::query!(
            r"
            DELETE FROM admin.amazon_sp_credentials
            WHERE account_name = $1
            ",
            account_name
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!("Amazon SP-API credentials deleted");
        } else {
            debug!("Amazon SP-API credentials not found for deletion");
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
        debug!("Checking if Amazon SP-API credentials exist");

        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM admin.amazon_sp_credentials WHERE account_name = $1
            ) as "exists!"
            "#,
            account_name
        )
        .fetch_one(self.pool)
        .await?;

        debug!(exists = %exists, "Amazon SP-API credentials existence check complete");

        Ok(exists)
    }
}

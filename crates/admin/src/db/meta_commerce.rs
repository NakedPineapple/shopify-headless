//! Meta Commerce credentials repository for database operations.
//!
//! This module provides database access for storing and retrieving
//! Meta Commerce API credentials (Facebook App + Page Access Token).

use chrono::{DateTime, Utc};
use secrecy::SecretString;
use sqlx::PgPool;
use tracing::{debug, info, instrument};

use super::RepositoryError;

// =============================================================================
// Types
// =============================================================================

/// Meta Commerce credentials stored in the database.
///
/// Implements `Debug` manually to redact sensitive tokens.
#[derive(Clone)]
pub struct MetaCommerceCredentials {
    /// Database ID.
    pub id: i32,
    /// Account name (default: "default").
    pub account_name: String,
    /// Facebook App ID.
    pub app_id: String,
    /// Facebook App Secret (HIGH PRIVILEGE - redacted in debug output).
    pub app_secret: SecretString,
    /// Page Access Token (HIGH PRIVILEGE - redacted in debug output).
    pub page_access_token: SecretString,
    /// Facebook Page ID.
    pub page_id: String,
    /// Commerce Account ID.
    pub commerce_account_id: String,
    /// Product Catalog ID.
    pub catalog_id: String,
    /// Unix timestamp when token expires.
    pub token_expires_at: Option<i64>,
    /// Admin user ID who connected the account.
    pub connected_by: Option<i32>,
    /// When the account was connected.
    pub connected_at: DateTime<Utc>,
    /// Last successful API call.
    pub last_used_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for MetaCommerceCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetaCommerceCredentials")
            .field("id", &self.id)
            .field("account_name", &self.account_name)
            .field("app_id", &self.app_id)
            .field("app_secret", &"[REDACTED]")
            .field("page_access_token", &"[REDACTED]")
            .field("page_id", &self.page_id)
            .field("commerce_account_id", &self.commerce_account_id)
            .field("catalog_id", &self.catalog_id)
            .field("token_expires_at", &self.token_expires_at)
            .field("connected_by", &self.connected_by)
            .field("connected_at", &self.connected_at)
            .field("last_used_at", &self.last_used_at)
            .finish()
    }
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct MetaCommerceCredentialsRow {
    id: i32,
    account_name: String,
    app_id: String,
    app_secret: String,
    page_access_token: String,
    page_id: String,
    commerce_account_id: String,
    catalog_id: String,
    token_expires_at: Option<i64>,
    connected_by: Option<i32>,
    connected_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
}

impl From<MetaCommerceCredentialsRow> for MetaCommerceCredentials {
    fn from(row: MetaCommerceCredentialsRow) -> Self {
        Self {
            id: row.id,
            account_name: row.account_name,
            app_id: row.app_id,
            app_secret: SecretString::from(row.app_secret),
            page_access_token: SecretString::from(row.page_access_token),
            page_id: row.page_id,
            commerce_account_id: row.commerce_account_id,
            catalog_id: row.catalog_id,
            token_expires_at: row.token_expires_at,
            connected_by: row.connected_by,
            connected_at: row.connected_at,
            last_used_at: row.last_used_at,
        }
    }
}

/// Parameters for saving Meta Commerce credentials.
pub struct SaveMetaCommerceParams<'a> {
    /// Account name (use "default" for the default account).
    pub account_name: &'a str,
    /// Facebook App ID.
    pub app_id: &'a str,
    /// Facebook App Secret.
    pub app_secret: &'a str,
    /// Page Access Token.
    pub page_access_token: &'a str,
    /// Facebook Page ID.
    pub page_id: &'a str,
    /// Commerce Account ID.
    pub commerce_account_id: &'a str,
    /// Product Catalog ID.
    pub catalog_id: &'a str,
    /// Admin user ID who connected the account.
    pub connected_by: Option<i32>,
}

impl std::fmt::Debug for SaveMetaCommerceParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SaveMetaCommerceParams")
            .field("account_name", &self.account_name)
            .field("app_id", &self.app_id)
            .field("app_secret", &"[REDACTED]")
            .field("page_access_token", &"[REDACTED]")
            .field("page_id", &self.page_id)
            .field("commerce_account_id", &self.commerce_account_id)
            .field("catalog_id", &self.catalog_id)
            .field("connected_by", &self.connected_by)
            .finish()
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for Meta Commerce credentials database operations.
pub struct MetaCommerceCredentialsRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> MetaCommerceCredentialsRepository<'a> {
    /// Create a new Meta Commerce credentials repository.
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
    pub async fn get_default(&self) -> Result<Option<MetaCommerceCredentials>, RepositoryError> {
        debug!("Fetching default Meta Commerce credentials");

        let row = sqlx::query_as!(
            MetaCommerceCredentialsRow,
            r#"
            SELECT
                id,
                account_name,
                app_id,
                app_secret,
                page_access_token,
                page_id,
                commerce_account_id,
                catalog_id,
                token_expires_at,
                connected_by,
                connected_at as "connected_at: DateTime<Utc>",
                last_used_at as "last_used_at: DateTime<Utc>"
            FROM admin.meta_commerce_credentials
            WHERE account_name = 'default'
            "#
        )
        .fetch_optional(self.pool)
        .await?;

        if row.is_none() {
            debug!("Meta Commerce credentials not found");
        }

        Ok(row.map(MetaCommerceCredentials::from))
    }

    /// Save or update credentials for an account.
    ///
    /// Uses upsert to handle both new and existing credentials.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(account_name = %params.account_name), level = "debug")]
    pub async fn save(&self, params: &SaveMetaCommerceParams<'_>) -> Result<(), RepositoryError> {
        debug!("Saving Meta Commerce credentials");

        sqlx::query!(
            r#"
            INSERT INTO admin.meta_commerce_credentials (
                account_name,
                app_id,
                app_secret,
                page_access_token,
                page_id,
                commerce_account_id,
                catalog_id,
                connected_by,
                connected_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, (CURRENT_TIMESTAMP AT TIME ZONE 'utc'))
            ON CONFLICT(account_name) DO UPDATE SET
                app_id = EXCLUDED.app_id,
                app_secret = EXCLUDED.app_secret,
                page_access_token = EXCLUDED.page_access_token,
                page_id = EXCLUDED.page_id,
                commerce_account_id = EXCLUDED.commerce_account_id,
                catalog_id = EXCLUDED.catalog_id,
                connected_by = EXCLUDED.connected_by,
                connected_at = EXCLUDED.connected_at,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            "#,
            params.account_name,
            params.app_id,
            params.app_secret,
            params.page_access_token,
            params.page_id,
            params.commerce_account_id,
            params.catalog_id,
            params.connected_by
        )
        .execute(self.pool)
        .await?;

        info!("Meta Commerce credentials saved");

        Ok(())
    }

    /// Update the cached token expiry.
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
        debug!("Updating Meta Commerce access token");

        let result = sqlx::query!(
            r#"
            UPDATE admin.meta_commerce_credentials
            SET
                page_access_token = $2,
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
            info!("Meta Commerce access token updated");
        } else {
            debug!("Meta Commerce credentials not found for token update");
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
        debug!("Updating Meta Commerce last_used_at timestamp");

        sqlx::query!(
            r"
            UPDATE admin.meta_commerce_credentials
            SET last_used_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            WHERE account_name = $1
            ",
            account_name
        )
        .execute(self.pool)
        .await?;

        debug!("Meta Commerce last_used_at timestamp updated");

        Ok(())
    }

    /// Delete credentials for an account.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(account_name = %account_name), level = "debug")]
    pub async fn delete(&self, account_name: &str) -> Result<bool, RepositoryError> {
        debug!("Deleting Meta Commerce credentials");

        let result = sqlx::query!(
            r"
            DELETE FROM admin.meta_commerce_credentials
            WHERE account_name = $1
            ",
            account_name
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!("Meta Commerce credentials deleted");
        } else {
            debug!("Meta Commerce credentials not found for deletion");
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
        debug!("Checking if Meta Commerce credentials exist");

        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM admin.meta_commerce_credentials WHERE account_name = $1
            ) as "exists!"
            "#,
            account_name
        )
        .fetch_one(self.pool)
        .await?;

        debug!(exists = %exists, "Meta Commerce credentials existence check complete");

        Ok(exists)
    }
}

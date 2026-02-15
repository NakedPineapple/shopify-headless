//! Faire credentials repository for database operations.
//!
//! This module provides database access for storing and retrieving
//! Faire API credentials (brand ID + API token, no OAuth).

use chrono::{DateTime, Utc};
use secrecy::SecretString;
use sqlx::PgPool;
use tracing::{debug, info, instrument};

use super::RepositoryError;

// =============================================================================
// Types
// =============================================================================

/// Faire credentials stored in the database.
///
/// Implements `Debug` manually to redact the sensitive API token.
#[derive(Clone)]
pub struct FaireCredentials {
    /// Database ID.
    pub id: i32,
    /// Account name (default: "default").
    pub account_name: String,
    /// Faire brand ID.
    pub brand_id: String,
    /// Faire API token (HIGH PRIVILEGE - redacted in debug output).
    pub api_token: SecretString,
    /// Admin user ID who connected the account.
    pub connected_by: Option<i32>,
    /// When the account was connected.
    pub connected_at: DateTime<Utc>,
    /// Last successful API call.
    pub last_used_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for FaireCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FaireCredentials")
            .field("id", &self.id)
            .field("account_name", &self.account_name)
            .field("brand_id", &self.brand_id)
            .field("api_token", &"[REDACTED]")
            .field("connected_by", &self.connected_by)
            .field("connected_at", &self.connected_at)
            .field("last_used_at", &self.last_used_at)
            .finish()
    }
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct FaireCredentialsRow {
    id: i32,
    account_name: String,
    brand_id: String,
    api_token: String,
    connected_by: Option<i32>,
    connected_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
}

impl From<FaireCredentialsRow> for FaireCredentials {
    fn from(row: FaireCredentialsRow) -> Self {
        Self {
            id: row.id,
            account_name: row.account_name,
            brand_id: row.brand_id,
            api_token: SecretString::from(row.api_token),
            connected_by: row.connected_by,
            connected_at: row.connected_at,
            last_used_at: row.last_used_at,
        }
    }
}

/// Parameters for saving Faire credentials.
pub struct SaveFaireParams<'a> {
    /// Account name (use "default" for the default account).
    pub account_name: &'a str,
    /// Faire brand ID.
    pub brand_id: &'a str,
    /// Faire API token.
    pub api_token: &'a str,
    /// Admin user ID who connected the account.
    pub connected_by: Option<i32>,
}

impl std::fmt::Debug for SaveFaireParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SaveFaireParams")
            .field("account_name", &self.account_name)
            .field("brand_id", &self.brand_id)
            .field("api_token", &"[REDACTED]")
            .field("connected_by", &self.connected_by)
            .finish()
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for Faire credentials database operations.
pub struct FaireCredentialsRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> FaireCredentialsRepository<'a> {
    /// Create a new Faire credentials repository.
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
    pub async fn get_default(&self) -> Result<Option<FaireCredentials>, RepositoryError> {
        debug!("Fetching default Faire credentials");

        let row = sqlx::query_as!(
            FaireCredentialsRow,
            r#"
            SELECT
                id,
                account_name,
                brand_id,
                api_token,
                connected_by,
                connected_at as "connected_at: DateTime<Utc>",
                last_used_at as "last_used_at: DateTime<Utc>"
            FROM admin.faire_credentials
            WHERE account_name = 'default'
            "#
        )
        .fetch_optional(self.pool)
        .await?;

        if row.is_none() {
            debug!("Faire credentials not found");
        }

        Ok(row.map(FaireCredentials::from))
    }

    /// Save or update credentials for an account.
    ///
    /// Uses upsert to handle both new and existing credentials.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(account_name = %params.account_name), level = "debug")]
    pub async fn save(&self, params: &SaveFaireParams<'_>) -> Result<(), RepositoryError> {
        debug!("Saving Faire credentials");

        sqlx::query!(
            r#"
            INSERT INTO admin.faire_credentials (
                account_name,
                brand_id,
                api_token,
                connected_by,
                connected_at
            )
            VALUES ($1, $2, $3, $4, (CURRENT_TIMESTAMP AT TIME ZONE 'utc'))
            ON CONFLICT(account_name) DO UPDATE SET
                brand_id = EXCLUDED.brand_id,
                api_token = EXCLUDED.api_token,
                connected_by = EXCLUDED.connected_by,
                connected_at = EXCLUDED.connected_at,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            "#,
            params.account_name,
            params.brand_id,
            params.api_token,
            params.connected_by
        )
        .execute(self.pool)
        .await?;

        info!("Faire credentials saved");

        Ok(())
    }

    /// Update the `last_used_at` timestamp.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(account_name = %account_name), level = "debug")]
    pub async fn touch(&self, account_name: &str) -> Result<(), RepositoryError> {
        debug!("Updating Faire last_used_at timestamp");

        sqlx::query!(
            r"
            UPDATE admin.faire_credentials
            SET last_used_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            WHERE account_name = $1
            ",
            account_name
        )
        .execute(self.pool)
        .await?;

        debug!("Faire last_used_at timestamp updated");

        Ok(())
    }

    /// Delete credentials for an account.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(account_name = %account_name), level = "debug")]
    pub async fn delete(&self, account_name: &str) -> Result<bool, RepositoryError> {
        debug!("Deleting Faire credentials");

        let result = sqlx::query!(
            r"
            DELETE FROM admin.faire_credentials
            WHERE account_name = $1
            ",
            account_name
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!("Faire credentials deleted");
        } else {
            debug!("Faire credentials not found for deletion");
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
        debug!("Checking if Faire credentials exist");

        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM admin.faire_credentials WHERE account_name = $1
            ) as "exists!"
            "#,
            account_name
        )
        .fetch_one(self.pool)
        .await?;

        debug!(exists = %exists, "Faire credentials existence check complete");

        Ok(exists)
    }
}

//! Shopify OAuth token repository for database operations.
//!
//! This module provides database access for storing and retrieving
//! Shopify Admin API OAuth tokens.

use chrono::{DateTime, Utc};
use secrecy::SecretString;
use sqlx::PgPool;
use tracing::{debug, info, instrument};

use super::RepositoryError;

// =============================================================================
// Types
// =============================================================================

/// A Shopify OAuth token.
///
/// Implements `Debug` manually to redact the access token.
#[derive(Clone)]
pub struct ShopifyToken {
    /// Shop domain (e.g., your-store.myshopify.com).
    pub shop: String,
    /// OAuth access token (HIGH PRIVILEGE - redacted in debug output).
    pub access_token: SecretString,
    /// Granted scopes.
    pub scopes: Vec<String>,
    /// Unix timestamp when token was obtained.
    pub obtained_at: i64,
}

impl std::fmt::Debug for ShopifyToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShopifyToken")
            .field("shop", &self.shop)
            .field("access_token", &"[REDACTED]")
            .field("scopes", &self.scopes)
            .field("obtained_at", &self.obtained_at)
            .finish()
    }
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct ShopifyTokenRow {
    id: i32,
    shop: String,
    access_token: String,
    scope: String,
    obtained_at: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ShopifyTokenRow> for ShopifyToken {
    fn from(row: ShopifyTokenRow) -> Self {
        let scopes = row
            .scope
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Self {
            shop: row.shop,
            access_token: SecretString::from(row.access_token),
            scopes,
            obtained_at: row.obtained_at,
        }
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for Shopify OAuth token database operations.
pub struct ShopifyTokenRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> ShopifyTokenRepository<'a> {
    /// Create a new Shopify token repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Get a token for a shop.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(shop = %shop), level = "debug")]
    pub async fn get_by_shop(&self, shop: &str) -> Result<Option<ShopifyToken>, RepositoryError> {
        debug!("Fetching Shopify token");

        let row = sqlx::query_as!(
            ShopifyTokenRow,
            r#"
            SELECT
                id,
                shop,
                access_token,
                scope,
                obtained_at,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.shopify_token
            WHERE shop = $1
            "#,
            shop
        )
        .fetch_optional(self.pool)
        .await?;

        if row.is_none() {
            debug!("Shopify token not found");
        }

        Ok(row.map(ShopifyToken::from))
    }

    /// Save or update a token for a shop.
    ///
    /// Uses upsert to handle both new and existing tokens.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, access_token), fields(shop = %shop), level = "debug")]
    pub async fn save(
        &self,
        shop: &str,
        access_token: &str,
        scopes: &[String],
        obtained_at: i64,
    ) -> Result<(), RepositoryError> {
        debug!("Saving Shopify token");

        let scope = scopes.join(",");

        sqlx::query!(
            r#"
            INSERT INTO admin.shopify_token (shop, access_token, scope, obtained_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT(shop) DO UPDATE SET
                access_token = EXCLUDED.access_token,
                scope = EXCLUDED.scope,
                obtained_at = EXCLUDED.obtained_at,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            "#,
            shop,
            access_token,
            scope,
            obtained_at
        )
        .execute(self.pool)
        .await?;

        info!("Shopify token saved");

        Ok(())
    }

    /// Delete a token for a shop.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(shop = %shop), level = "debug")]
    pub async fn delete(&self, shop: &str) -> Result<bool, RepositoryError> {
        debug!("Deleting Shopify token");

        let result = sqlx::query!(
            r#"
            DELETE FROM admin.shopify_token
            WHERE shop = $1
            "#,
            shop
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!("Shopify token deleted");
        } else {
            debug!("Shopify token not found for deletion");
        }

        Ok(deleted)
    }

    /// Check if a shop has a token stored.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(shop = %shop), level = "debug")]
    pub async fn exists(&self, shop: &str) -> Result<bool, RepositoryError> {
        debug!("Checking if Shopify token exists");

        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM admin.shopify_token WHERE shop = $1
            ) as "exists!"
            "#,
            shop
        )
        .fetch_one(self.pool)
        .await?;

        debug!(exists = %exists, "Shopify token existence check complete");

        Ok(exists)
    }
}

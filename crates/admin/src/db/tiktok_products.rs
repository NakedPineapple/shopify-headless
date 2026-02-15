//! TikTok Shop product mapping repository.
//!
//! Maps Shopify products/variants to TikTok Shop product and SKU IDs,
//! enabling cross-channel catalog management for TikTok Shop.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::{debug, info, instrument};

use super::RepositoryError;

// =============================================================================
// Types
// =============================================================================

/// A Shopify-to-TikTok Shop product mapping.
#[derive(Debug, Clone)]
pub struct TikTokProductMapping {
    pub id: i32,
    pub shopify_product_id: String,
    pub shopify_variant_id: Option<String>,
    pub tiktok_product_id: String,
    pub tiktok_sku_id: Option<String>,
    pub match_type: String,
    pub status: String,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct MappingRow {
    id: i32,
    shopify_product_id: String,
    shopify_variant_id: Option<String>,
    tiktok_product_id: String,
    tiktok_sku_id: Option<String>,
    match_type: String,
    status: String,
    last_sync_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<MappingRow> for TikTokProductMapping {
    fn from(row: MappingRow) -> Self {
        Self {
            id: row.id,
            shopify_product_id: row.shopify_product_id,
            shopify_variant_id: row.shopify_variant_id,
            tiktok_product_id: row.tiktok_product_id,
            tiktok_sku_id: row.tiktok_sku_id,
            match_type: row.match_type,
            status: row.status,
            last_sync_at: row.last_sync_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Parameters for creating a TikTok product mapping.
#[derive(Debug)]
pub struct CreateTikTokMappingParams<'a> {
    pub shopify_product_id: &'a str,
    pub shopify_variant_id: Option<&'a str>,
    pub tiktok_product_id: &'a str,
    pub tiktok_sku_id: Option<&'a str>,
    pub match_type: &'a str,
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for TikTok product mapping database operations.
pub struct TikTokProductMappingRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> TikTokProductMappingRepository<'a> {
    /// Create a new product mapping repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// List all product mappings, ordered by most recent first.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn list_all(&self) -> Result<Vec<TikTokProductMapping>, RepositoryError> {
        debug!("Listing TikTok product mappings");

        let rows = sqlx::query_as!(
            MappingRow,
            r#"
            SELECT
                id,
                shopify_product_id,
                shopify_variant_id,
                tiktok_product_id,
                tiktok_sku_id,
                match_type,
                status,
                last_sync_at as "last_sync_at: DateTime<Utc>",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.tiktok_product_mapping
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(TikTokProductMapping::from).collect())
    }

    /// Get mappings by Shopify product ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_by_shopify_product(
        &self,
        shopify_product_id: &str,
    ) -> Result<Vec<TikTokProductMapping>, RepositoryError> {
        debug!("Fetching TikTok mapping by Shopify product ID");

        let rows = sqlx::query_as!(
            MappingRow,
            r#"
            SELECT
                id,
                shopify_product_id,
                shopify_variant_id,
                tiktok_product_id,
                tiktok_sku_id,
                match_type,
                status,
                last_sync_at as "last_sync_at: DateTime<Utc>",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.tiktok_product_mapping
            WHERE shopify_product_id = $1
            "#,
            shopify_product_id
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(TikTokProductMapping::from).collect())
    }

    /// Get a mapping by TikTok product ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_by_tiktok_product(
        &self,
        tiktok_product_id: &str,
    ) -> Result<Option<TikTokProductMapping>, RepositoryError> {
        debug!("Fetching TikTok mapping by TikTok product ID");

        let row = sqlx::query_as!(
            MappingRow,
            r#"
            SELECT
                id,
                shopify_product_id,
                shopify_variant_id,
                tiktok_product_id,
                tiktok_sku_id,
                match_type,
                status,
                last_sync_at as "last_sync_at: DateTime<Utc>",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.tiktok_product_mapping
            WHERE tiktok_product_id = $1
            "#,
            tiktok_product_id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(TikTokProductMapping::from))
    }

    /// Create a new product mapping.
    ///
    /// Uses upsert on `tiktok_product_id` to prevent duplicates.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(tiktok_product = %params.tiktok_product_id), level = "debug")]
    pub async fn create(
        &self,
        params: &CreateTikTokMappingParams<'_>,
    ) -> Result<TikTokProductMapping, RepositoryError> {
        debug!("Creating TikTok product mapping");

        let row = sqlx::query_as!(
            MappingRow,
            r#"
            INSERT INTO admin.tiktok_product_mapping (
                shopify_product_id,
                shopify_variant_id,
                tiktok_product_id,
                tiktok_sku_id,
                match_type
            )
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (tiktok_product_id) DO UPDATE SET
                shopify_product_id = EXCLUDED.shopify_product_id,
                shopify_variant_id = EXCLUDED.shopify_variant_id,
                tiktok_sku_id = EXCLUDED.tiktok_sku_id,
                match_type = EXCLUDED.match_type,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            RETURNING
                id,
                shopify_product_id,
                shopify_variant_id,
                tiktok_product_id,
                tiktok_sku_id,
                match_type,
                status,
                last_sync_at as "last_sync_at: DateTime<Utc>",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            "#,
            params.shopify_product_id,
            params.shopify_variant_id,
            params.tiktok_product_id,
            params.tiktok_sku_id,
            params.match_type
        )
        .fetch_one(self.pool)
        .await?;

        info!(id = row.id, "TikTok product mapping created");

        Ok(TikTokProductMapping::from(row))
    }

    /// Delete a product mapping by ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
        debug!("Deleting TikTok product mapping");

        let result = sqlx::query!(
            r"DELETE FROM admin.tiktok_product_mapping WHERE id = $1",
            id
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!(id = id, "TikTok product mapping deleted");
        }

        Ok(deleted)
    }

    /// Count all product mappings.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn count(&self) -> Result<i64, RepositoryError> {
        let count =
            sqlx::query_scalar!(r#"SELECT COUNT(*) as "count!" FROM admin.tiktok_product_mapping"#)
                .fetch_one(self.pool)
                .await?;

        Ok(count)
    }
}

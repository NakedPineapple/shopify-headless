//! Amazon product mapping repository.
//!
//! Maps Shopify products/variants to Amazon ASINs and seller SKUs,
//! enabling cross-channel catalog visibility.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::{debug, info, instrument};

use super::RepositoryError;

// =============================================================================
// Types
// =============================================================================

/// A Shopify-to-Amazon product mapping.
#[derive(Debug, Clone)]
pub struct AmazonProductMapping {
    pub id: i32,
    pub shopify_product_id: String,
    pub shopify_variant_id: Option<String>,
    pub asin: String,
    pub amazon_sku: String,
    pub marketplace_id: String,
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
    asin: String,
    amazon_sku: String,
    marketplace_id: String,
    match_type: String,
    status: String,
    last_sync_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<MappingRow> for AmazonProductMapping {
    fn from(row: MappingRow) -> Self {
        Self {
            id: row.id,
            shopify_product_id: row.shopify_product_id,
            shopify_variant_id: row.shopify_variant_id,
            asin: row.asin,
            amazon_sku: row.amazon_sku,
            marketplace_id: row.marketplace_id,
            match_type: row.match_type,
            status: row.status,
            last_sync_at: row.last_sync_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Parameters for creating a product mapping.
#[derive(Debug)]
pub struct CreateMappingParams<'a> {
    pub shopify_product_id: &'a str,
    pub shopify_variant_id: Option<&'a str>,
    pub asin: &'a str,
    pub amazon_sku: &'a str,
    pub marketplace_id: &'a str,
    pub match_type: &'a str,
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for Amazon product mapping database operations.
pub struct AmazonProductMappingRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> AmazonProductMappingRepository<'a> {
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
    pub async fn list(&self) -> Result<Vec<AmazonProductMapping>, RepositoryError> {
        debug!("Listing Amazon product mappings");

        let rows = sqlx::query_as!(
            MappingRow,
            r#"
            SELECT
                id,
                shopify_product_id,
                shopify_variant_id,
                asin,
                amazon_sku,
                marketplace_id,
                match_type,
                status,
                last_sync_at as "last_sync_at: DateTime<Utc>",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.amazon_product_mapping
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(AmazonProductMapping::from).collect())
    }

    /// Get a product mapping by ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get(&self, id: i32) -> Result<Option<AmazonProductMapping>, RepositoryError> {
        debug!("Fetching Amazon product mapping");

        let row = sqlx::query_as!(
            MappingRow,
            r#"
            SELECT
                id,
                shopify_product_id,
                shopify_variant_id,
                asin,
                amazon_sku,
                marketplace_id,
                match_type,
                status,
                last_sync_at as "last_sync_at: DateTime<Utc>",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.amazon_product_mapping
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(AmazonProductMapping::from))
    }

    /// Create a new product mapping.
    ///
    /// Uses upsert on `(amazon_sku, marketplace_id)` to prevent duplicates.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(sku = %params.amazon_sku, asin = %params.asin), level = "debug")]
    pub async fn create(
        &self,
        params: &CreateMappingParams<'_>,
    ) -> Result<AmazonProductMapping, RepositoryError> {
        debug!("Creating Amazon product mapping");

        let row = sqlx::query_as!(
            MappingRow,
            r#"
            INSERT INTO admin.amazon_product_mapping (
                shopify_product_id,
                shopify_variant_id,
                asin,
                amazon_sku,
                marketplace_id,
                match_type
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (amazon_sku, marketplace_id) DO UPDATE SET
                shopify_product_id = EXCLUDED.shopify_product_id,
                shopify_variant_id = EXCLUDED.shopify_variant_id,
                asin = EXCLUDED.asin,
                match_type = EXCLUDED.match_type,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            RETURNING
                id,
                shopify_product_id,
                shopify_variant_id,
                asin,
                amazon_sku,
                marketplace_id,
                match_type,
                status,
                last_sync_at as "last_sync_at: DateTime<Utc>",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            "#,
            params.shopify_product_id,
            params.shopify_variant_id,
            params.asin,
            params.amazon_sku,
            params.marketplace_id,
            params.match_type
        )
        .fetch_one(self.pool)
        .await?;

        info!(id = row.id, "Amazon product mapping created");

        Ok(AmazonProductMapping::from(row))
    }

    /// Delete a product mapping by ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
        debug!("Deleting Amazon product mapping");

        let result = sqlx::query!(
            r"DELETE FROM admin.amazon_product_mapping WHERE id = $1",
            id
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!(id = id, "Amazon product mapping deleted");
        }

        Ok(deleted)
    }

    /// Get a mapping by Amazon SKU and marketplace.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_by_sku(
        &self,
        amazon_sku: &str,
        marketplace_id: &str,
    ) -> Result<Option<AmazonProductMapping>, RepositoryError> {
        debug!("Fetching mapping by Amazon SKU");

        let row = sqlx::query_as!(
            MappingRow,
            r#"
            SELECT
                id,
                shopify_product_id,
                shopify_variant_id,
                asin,
                amazon_sku,
                marketplace_id,
                match_type,
                status,
                last_sync_at as "last_sync_at: DateTime<Utc>",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.amazon_product_mapping
            WHERE amazon_sku = $1 AND marketplace_id = $2
            "#,
            amazon_sku,
            marketplace_id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(AmazonProductMapping::from))
    }

    /// Count all product mappings.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn count(&self) -> Result<i64, RepositoryError> {
        let count =
            sqlx::query_scalar!(r#"SELECT COUNT(*) as "count!" FROM admin.amazon_product_mapping"#)
                .fetch_one(self.pool)
                .await?;

        Ok(count)
    }
}

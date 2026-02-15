//! Google Merchant Center product mapping repository.
//!
//! Maps Shopify products/variants to Google Merchant Center product IDs,
//! enabling cross-channel catalog visibility for Google Shopping and `YouTube` Shopping.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::{debug, info, instrument};

use super::RepositoryError;

// =============================================================================
// Types
// =============================================================================

/// A Shopify-to-Google product mapping.
#[derive(Debug, Clone)]
pub struct GoogleProductMapping {
    pub id: i32,
    pub shopify_product_id: String,
    pub shopify_variant_id: Option<String>,
    pub google_product_id: String,
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
    google_product_id: String,
    match_type: String,
    status: String,
    last_sync_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<MappingRow> for GoogleProductMapping {
    fn from(row: MappingRow) -> Self {
        Self {
            id: row.id,
            shopify_product_id: row.shopify_product_id,
            shopify_variant_id: row.shopify_variant_id,
            google_product_id: row.google_product_id,
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
pub struct CreateGoogleMappingParams<'a> {
    pub shopify_product_id: &'a str,
    pub shopify_variant_id: Option<&'a str>,
    pub google_product_id: &'a str,
    pub match_type: &'a str,
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for Google product mapping database operations.
pub struct GoogleProductMappingRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> GoogleProductMappingRepository<'a> {
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
    pub async fn list_all(&self) -> Result<Vec<GoogleProductMapping>, RepositoryError> {
        debug!("Listing Google product mappings");

        let rows = sqlx::query_as!(
            MappingRow,
            r#"
            SELECT
                id,
                shopify_product_id,
                shopify_variant_id,
                google_product_id,
                match_type,
                status,
                last_sync_at as "last_sync_at: DateTime<Utc>",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.google_product_mapping
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(GoogleProductMapping::from).collect())
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
    ) -> Result<Vec<GoogleProductMapping>, RepositoryError> {
        debug!("Fetching Google mapping by Shopify product ID");

        let rows = sqlx::query_as!(
            MappingRow,
            r#"
            SELECT
                id,
                shopify_product_id,
                shopify_variant_id,
                google_product_id,
                match_type,
                status,
                last_sync_at as "last_sync_at: DateTime<Utc>",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.google_product_mapping
            WHERE shopify_product_id = $1
            "#,
            shopify_product_id
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(GoogleProductMapping::from).collect())
    }

    /// Create a new product mapping.
    ///
    /// Uses upsert on `google_product_id` to prevent duplicates.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(google_product = %params.google_product_id), level = "debug")]
    pub async fn create(
        &self,
        params: &CreateGoogleMappingParams<'_>,
    ) -> Result<GoogleProductMapping, RepositoryError> {
        debug!("Creating Google product mapping");

        let row = sqlx::query_as!(
            MappingRow,
            r#"
            INSERT INTO admin.google_product_mapping (
                shopify_product_id,
                shopify_variant_id,
                google_product_id,
                match_type
            )
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (google_product_id) DO UPDATE SET
                shopify_product_id = EXCLUDED.shopify_product_id,
                shopify_variant_id = EXCLUDED.shopify_variant_id,
                match_type = EXCLUDED.match_type,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            RETURNING
                id,
                shopify_product_id,
                shopify_variant_id,
                google_product_id,
                match_type,
                status,
                last_sync_at as "last_sync_at: DateTime<Utc>",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            "#,
            params.shopify_product_id,
            params.shopify_variant_id,
            params.google_product_id,
            params.match_type
        )
        .fetch_one(self.pool)
        .await?;

        info!(id = row.id, "Google product mapping created");

        Ok(GoogleProductMapping::from(row))
    }

    /// Delete a product mapping by ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn delete(&self, id: i32) -> Result<bool, RepositoryError> {
        debug!("Deleting Google product mapping");

        let result = sqlx::query!(
            r"DELETE FROM admin.google_product_mapping WHERE id = $1",
            id
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!(id = id, "Google product mapping deleted");
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
            sqlx::query_scalar!(r#"SELECT COUNT(*) as "count!" FROM admin.google_product_mapping"#)
                .fetch_one(self.pool)
                .await?;

        Ok(count)
    }
}

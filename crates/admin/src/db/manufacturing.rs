//! Database operations for manufacturing batches.
//!
//! All queries use sqlx macros for compile-time verification.

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use naked_pineapple_core::{BatchMetadataId, ManufacturingBatchId};

/// Convert chrono `NaiveDate` to `time::Date` for `SQLx` compatibility.
///
/// This conversion is necessary due to `SQLx`'s type resolution when both `chrono` and `time`
/// crates are present in the dependency graph. Even though `SQLx` has the `chrono` feature
/// enabled (which maps `PostgreSQL` `DATE` to `chrono::NaiveDate`), the `time` crate is pulled
/// in transitively by `webauthn-rs`, `tower-sessions`, `reqwest`, and other dependencies.
///
/// `SQLx` exhibits asymmetric behavior in this situation:
/// - **Reading** (SELECT): Works with chrono via explicit type annotations like
///   `as "manufacture_date: NaiveDate"`
/// - **Writing** (INSERT/UPDATE): Expects `time::Date` for bind parameters
///
/// This asymmetry only affects `DATE` columns. `TIMESTAMPTZ` columns (used for `created_at`,
/// `updated_at`, etc.) work fine with `DateTime<Utc>` because there's no ambiguous `time`
/// equivalent. The manufacturing tables are the first in the admin crate to use `DATE` columns.
///
/// We keep the public API using chrono types for consistency with the rest of the codebase,
/// and perform this conversion internally when binding parameters to INSERT/UPDATE queries.
fn to_time_date(date: NaiveDate) -> time::Date {
    let month = u8::try_from(date.month()).expect("month in range 1-12");
    let day = u8::try_from(date.day()).expect("day in range 1-31");
    time::Date::from_calendar_date(
        date.year(),
        time::Month::try_from(month).expect("valid month"),
        day,
    )
    .expect("valid date")
}

use super::RepositoryError;
use crate::models::manufacturing::{
    BatchFilter, BatchMetadata, CreateBatchInput, ManufacturingBatch, UpdateBatchInput,
};

// =============================================================================
// Internal Row Types
// =============================================================================

/// Internal row type for manufacturing batch queries.
#[derive(Debug, sqlx::FromRow)]
struct ManufacturingBatchRow {
    id: i32,
    batch_number: String,
    shopify_product_id: String,
    shopify_variant_id: Option<String>,
    quantity: i32,
    manufacture_date: NaiveDate,
    raw_cost_per_item: Decimal,
    label_cost_per_item: Decimal,
    outer_carton_cost_per_item: Decimal,
    cost_per_unit: Decimal,
    total_batch_cost: Decimal,
    currency_code: String,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ManufacturingBatchRow> for ManufacturingBatch {
    fn from(row: ManufacturingBatchRow) -> Self {
        Self {
            id: ManufacturingBatchId::new(row.id),
            batch_number: row.batch_number,
            shopify_product_id: row.shopify_product_id,
            shopify_variant_id: row.shopify_variant_id,
            quantity: row.quantity,
            manufacture_date: row.manufacture_date,
            raw_cost_per_item: row.raw_cost_per_item,
            label_cost_per_item: row.label_cost_per_item,
            outer_carton_cost_per_item: row.outer_carton_cost_per_item,
            cost_per_unit: row.cost_per_unit,
            total_batch_cost: row.total_batch_cost,
            currency_code: row.currency_code,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Internal row type for batch metadata queries.
#[derive(Debug, sqlx::FromRow)]
struct BatchMetadataRow {
    id: i32,
    batch_id: i32,
    key: String,
    value: serde_json::Value,
    created_at: DateTime<Utc>,
}

impl From<BatchMetadataRow> for BatchMetadata {
    fn from(row: BatchMetadataRow) -> Self {
        Self {
            id: BatchMetadataId::new(row.id),
            batch_id: ManufacturingBatchId::new(row.batch_id),
            key: row.key,
            value: row.value,
            created_at: row.created_at,
        }
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for manufacturing batch database operations.
pub struct ManufacturingRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> ManufacturingRepository<'a> {
    /// Create a new manufacturing repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // Batch CRUD
    // =========================================================================

    /// Create a new manufacturing batch.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Conflict` if a batch with the same number
    /// already exists for this product.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn create_batch(
        &self,
        input: &CreateBatchInput,
    ) -> Result<ManufacturingBatch, RepositoryError> {
        let row = sqlx::query_as!(
            ManufacturingBatchRow,
            r#"
            INSERT INTO admin.manufacturing_batch (
                batch_number, shopify_product_id, shopify_variant_id,
                quantity, manufacture_date,
                raw_cost_per_item, label_cost_per_item, outer_carton_cost_per_item,
                currency_code, notes
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING
                id, batch_number, shopify_product_id, shopify_variant_id,
                quantity,
                manufacture_date as "manufacture_date: NaiveDate",
                raw_cost_per_item, label_cost_per_item, outer_carton_cost_per_item,
                cost_per_unit as "cost_per_unit!: Decimal",
                total_batch_cost as "total_batch_cost!: Decimal",
                currency_code, notes,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            "#,
            input.batch_number,
            input.shopify_product_id,
            input.shopify_variant_id,
            input.quantity,
            to_time_date(input.manufacture_date),
            input.raw_cost_per_item,
            input.label_cost_per_item,
            input.outer_carton_cost_per_item,
            input.currency_code,
            input.notes
        )
        .fetch_one(self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e
                && db_err.constraint() == Some("idx_manufacturing_batch_number_product")
            {
                return RepositoryError::Conflict(
                    "Batch number already exists for this product".to_string(),
                );
            }
            RepositoryError::Database(e)
        })?;

        Ok(row.into())
    }

    /// Get a manufacturing batch by ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get_batch(
        &self,
        id: ManufacturingBatchId,
    ) -> Result<Option<ManufacturingBatch>, RepositoryError> {
        let row = sqlx::query_as!(
            ManufacturingBatchRow,
            r#"
            SELECT
                id, batch_number, shopify_product_id, shopify_variant_id,
                quantity,
                manufacture_date as "manufacture_date: NaiveDate",
                raw_cost_per_item, label_cost_per_item, outer_carton_cost_per_item,
                cost_per_unit as "cost_per_unit!: Decimal",
                total_batch_cost as "total_batch_cost!: Decimal",
                currency_code, notes,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.manufacturing_batch
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    /// List manufacturing batches with optional filtering.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn list_batches(
        &self,
        filter: &BatchFilter,
    ) -> Result<Vec<ManufacturingBatch>, RepositoryError> {
        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);

        let rows = sqlx::query_as!(
            ManufacturingBatchRow,
            r#"
            SELECT
                id, batch_number, shopify_product_id, shopify_variant_id,
                quantity,
                manufacture_date as "manufacture_date: NaiveDate",
                raw_cost_per_item, label_cost_per_item, outer_carton_cost_per_item,
                cost_per_unit as "cost_per_unit!: Decimal",
                total_batch_cost as "total_batch_cost!: Decimal",
                currency_code, notes,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.manufacturing_batch
            WHERE
                ($1::text IS NULL OR shopify_product_id = $1)
                AND ($2::date IS NULL OR manufacture_date >= $2)
                AND ($3::date IS NULL OR manufacture_date <= $3)
                AND ($4::text IS NULL OR batch_number ILIKE '%' || $4 || '%')
            ORDER BY manufacture_date DESC, created_at DESC
            LIMIT $5 OFFSET $6
            "#,
            filter.shopify_product_id,
            filter.start_date.map(to_time_date),
            filter.end_date.map(to_time_date),
            filter.batch_number,
            limit,
            offset
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Update a manufacturing batch.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the batch doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn update_batch(
        &self,
        id: ManufacturingBatchId,
        input: &UpdateBatchInput,
    ) -> Result<ManufacturingBatch, RepositoryError> {
        let row = sqlx::query_as!(
            ManufacturingBatchRow,
            r#"
            UPDATE admin.manufacturing_batch
            SET
                batch_number = COALESCE($2, batch_number),
                quantity = COALESCE($3, quantity),
                manufacture_date = COALESCE($4, manufacture_date),
                raw_cost_per_item = COALESCE($5, raw_cost_per_item),
                label_cost_per_item = COALESCE($6, label_cost_per_item),
                outer_carton_cost_per_item = COALESCE($7, outer_carton_cost_per_item),
                currency_code = COALESCE($8, currency_code),
                notes = COALESCE($9, notes)
            WHERE id = $1
            RETURNING
                id, batch_number, shopify_product_id, shopify_variant_id,
                quantity,
                manufacture_date as "manufacture_date: NaiveDate",
                raw_cost_per_item, label_cost_per_item, outer_carton_cost_per_item,
                cost_per_unit as "cost_per_unit!: Decimal",
                total_batch_cost as "total_batch_cost!: Decimal",
                currency_code, notes,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            "#,
            id.as_i32(),
            input.batch_number,
            input.quantity,
            input.manufacture_date.map(to_time_date),
            input.raw_cost_per_item,
            input.label_cost_per_item,
            input.outer_carton_cost_per_item,
            input.currency_code,
            input.notes
        )
        .fetch_optional(self.pool)
        .await?
        .ok_or(RepositoryError::NotFound)?;

        Ok(row.into())
    }

    /// Delete a manufacturing batch.
    ///
    /// Note: This will fail if there are inventory lots referencing this batch
    /// (due to RESTRICT foreign key).
    ///
    /// # Returns
    ///
    /// Returns `true` if the batch was deleted, `false` if it didn't exist.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn delete_batch(&self, id: ManufacturingBatchId) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            r#"
            DELETE FROM admin.manufacturing_batch
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Count total batches matching filter.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn count_batches(&self, filter: &BatchFilter) -> Result<i64, RepositoryError> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM admin.manufacturing_batch
            WHERE
                ($1::text IS NULL OR shopify_product_id = $1)
                AND ($2::date IS NULL OR manufacture_date >= $2)
                AND ($3::date IS NULL OR manufacture_date <= $3)
                AND ($4::text IS NULL OR batch_number ILIKE '%' || $4 || '%')
            "#,
            filter.shopify_product_id,
            filter.start_date.map(to_time_date),
            filter.end_date.map(to_time_date),
            filter.batch_number
        )
        .fetch_one(self.pool)
        .await?;

        Ok(count)
    }

    /// Get total units received as lots for a batch.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get_lots_received(
        &self,
        batch_id: ManufacturingBatchId,
    ) -> Result<i64, RepositoryError> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(SUM(quantity), 0) as "count!"
            FROM admin.inventory_lot
            WHERE batch_id = $1
            "#,
            batch_id.as_i32()
        )
        .fetch_one(self.pool)
        .await?;

        Ok(count)
    }

    // =========================================================================
    // Metadata CRUD
    // =========================================================================

    /// Set a metadata key-value pair for a batch (upsert).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn set_metadata(
        &self,
        batch_id: ManufacturingBatchId,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<BatchMetadata, RepositoryError> {
        let row = sqlx::query_as!(
            BatchMetadataRow,
            r#"
            INSERT INTO admin.batch_metadata (batch_id, key, value)
            VALUES ($1, $2, $3)
            ON CONFLICT (batch_id, key) DO UPDATE
            SET value = EXCLUDED.value
            RETURNING
                id, batch_id, key, value,
                created_at as "created_at: DateTime<Utc>"
            "#,
            batch_id.as_i32(),
            key,
            value
        )
        .fetch_one(self.pool)
        .await?;

        Ok(row.into())
    }

    /// Get all metadata for a batch.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get_metadata(
        &self,
        batch_id: ManufacturingBatchId,
    ) -> Result<Vec<BatchMetadata>, RepositoryError> {
        let rows = sqlx::query_as!(
            BatchMetadataRow,
            r#"
            SELECT
                id, batch_id, key, value,
                created_at as "created_at: DateTime<Utc>"
            FROM admin.batch_metadata
            WHERE batch_id = $1
            ORDER BY key
            "#,
            batch_id.as_i32()
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Delete a metadata key from a batch.
    ///
    /// # Returns
    ///
    /// Returns `true` if the metadata was deleted, `false` if it didn't exist.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn delete_metadata(
        &self,
        batch_id: ManufacturingBatchId,
        key: &str,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            r#"
            DELETE FROM admin.batch_metadata
            WHERE batch_id = $1 AND key = $2
            "#,
            batch_id.as_i32(),
            key
        )
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

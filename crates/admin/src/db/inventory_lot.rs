//! Database operations for inventory lots and allocations.
//!
//! All queries use sqlx macros for compile-time verification.

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, info, instrument, warn};

use naked_pineapple_core::{AdminUserId, InventoryLotId, LotAllocationId, ManufacturingBatchId};

/// COGS (cost of goods sold) per product for margin reporting.
#[derive(Debug, Clone)]
pub struct ProductCogs {
    pub shopify_product_id: String,
    pub total_cogs: Decimal,
    pub units_allocated: i64,
}

/// COGS per order for margin reporting.
#[derive(Debug, Clone)]
pub struct OrderCogs {
    pub shopify_order_id: String,
    pub total_cogs: Decimal,
}

/// Convert chrono `NaiveDate` to `time::Date` for `SQLx` compatibility.
///
/// This conversion is necessary due to `SQLx`'s type resolution when both `chrono` and `time`
/// crates are present in the dependency graph. Even though `SQLx` has the `chrono` feature
/// enabled (which maps `PostgreSQL` `DATE` to `chrono::NaiveDate`), the `time` crate is pulled
/// in transitively by `webauthn-rs`, `tower-sessions`, `reqwest`, and other dependencies.
///
/// `SQLx` exhibits asymmetric behavior in this situation:
/// - **Reading** (SELECT): Works with chrono via explicit type annotations like
///   `as "received_date: NaiveDate"`
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

/// Convert chrono `NaiveDate` to `time::OffsetDateTime` at midnight UTC for `SQLx` TIMESTAMPTZ binds.
fn to_time_offset_midnight(date: NaiveDate) -> time::OffsetDateTime {
    to_time_date(date).midnight().assume_utc()
}

/// Convert chrono `NaiveDate` to `time::OffsetDateTime` at midnight of the *next* day (exclusive upper bound).
fn to_time_offset_next_midnight(date: NaiveDate) -> time::OffsetDateTime {
    to_time_date(date)
        .next_day()
        .expect("valid date")
        .midnight()
        .assume_utc()
}

use super::RepositoryError;
use crate::models::inventory_lot::{
    AllocateLotInput, CreateLotInput, InventoryLot, InventoryLotWithBatch,
    InventoryLotWithRemaining, LotAllocation, LotFilter, UpdateLotInput,
};

// =============================================================================
// Internal Row Types
// =============================================================================

/// Internal row type for inventory lot queries.
#[derive(Debug, sqlx::FromRow)]
struct InventoryLotRow {
    id: i32,
    batch_id: i32,
    lot_number: String,
    quantity: i32,
    received_date: NaiveDate,
    shopify_location_id: Option<String>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<InventoryLotRow> for InventoryLot {
    fn from(row: InventoryLotRow) -> Self {
        Self {
            id: InventoryLotId::new(row.id),
            batch_id: ManufacturingBatchId::new(row.batch_id),
            lot_number: row.lot_number,
            quantity: row.quantity,
            received_date: row.received_date,
            shopify_location_id: row.shopify_location_id,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Internal row type for lot with remaining quantity.
#[derive(Debug, sqlx::FromRow)]
struct InventoryLotWithRemainingRow {
    id: i32,
    batch_id: i32,
    lot_number: String,
    quantity: i32,
    received_date: NaiveDate,
    shopify_location_id: Option<String>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    quantity_remaining: i64,
}

impl From<InventoryLotWithRemainingRow> for InventoryLotWithRemaining {
    fn from(row: InventoryLotWithRemainingRow) -> Self {
        Self {
            lot: InventoryLot {
                id: InventoryLotId::new(row.id),
                batch_id: ManufacturingBatchId::new(row.batch_id),
                lot_number: row.lot_number,
                quantity: row.quantity,
                received_date: row.received_date,
                shopify_location_id: row.shopify_location_id,
                notes: row.notes,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            quantity_remaining: row.quantity_remaining,
        }
    }
}

/// Internal row type for lot with batch info.
#[derive(Debug, sqlx::FromRow)]
struct InventoryLotWithBatchRow {
    id: i32,
    batch_id: i32,
    lot_number: String,
    quantity: i32,
    received_date: NaiveDate,
    shopify_location_id: Option<String>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    quantity_remaining: i64,
    batch_number: String,
    cost_per_unit: Decimal,
    currency_code: String,
}

impl From<InventoryLotWithBatchRow> for InventoryLotWithBatch {
    fn from(row: InventoryLotWithBatchRow) -> Self {
        Self {
            lot: InventoryLot {
                id: InventoryLotId::new(row.id),
                batch_id: ManufacturingBatchId::new(row.batch_id),
                lot_number: row.lot_number,
                quantity: row.quantity,
                received_date: row.received_date,
                shopify_location_id: row.shopify_location_id,
                notes: row.notes,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            quantity_remaining: row.quantity_remaining,
            batch_number: row.batch_number,
            cost_per_unit: row.cost_per_unit,
            currency_code: row.currency_code,
        }
    }
}

/// Internal row type for lot allocation queries.
#[derive(Debug, sqlx::FromRow)]
struct LotAllocationRow {
    id: i32,
    lot_id: i32,
    shopify_order_id: String,
    shopify_line_item_id: String,
    quantity: i32,
    allocated_at: DateTime<Utc>,
    allocated_by: Option<i32>,
}

impl From<LotAllocationRow> for LotAllocation {
    fn from(row: LotAllocationRow) -> Self {
        Self {
            id: LotAllocationId::new(row.id),
            lot_id: InventoryLotId::new(row.lot_id),
            shopify_order_id: row.shopify_order_id,
            shopify_line_item_id: row.shopify_line_item_id,
            quantity: row.quantity,
            allocated_at: row.allocated_at,
            allocated_by: row.allocated_by.map(AdminUserId::new),
        }
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for inventory lot database operations.
pub struct InventoryLotRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> InventoryLotRepository<'a> {
    /// Create a new inventory lot repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // Lot CRUD
    // =========================================================================

    /// Create a new inventory lot.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, input), level = "debug", fields(batch_id = %input.batch_id.as_i32()))]
    pub async fn create_lot(
        &self,
        input: &CreateLotInput,
    ) -> Result<InventoryLot, RepositoryError> {
        debug!("Creating inventory lot");
        let row = sqlx::query_as!(
            InventoryLotRow,
            r#"
            INSERT INTO admin.inventory_lot (
                batch_id, lot_number, quantity, received_date,
                shopify_location_id, notes
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
                id, batch_id, lot_number, quantity,
                received_date as "received_date: NaiveDate",
                shopify_location_id, notes,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            "#,
            input.batch_id.as_i32(),
            input.lot_number,
            input.quantity,
            to_time_date(input.received_date),
            input.shopify_location_id,
            input.notes
        )
        .fetch_one(self.pool)
        .await?;

        let lot: InventoryLot = row.into();
        info!(lot_id = %lot.id.as_i32(), "Inventory lot created");
        Ok(lot)
    }

    /// Get an inventory lot by ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug", fields(lot_id = %id.as_i32()))]
    pub async fn get_lot(
        &self,
        id: InventoryLotId,
    ) -> Result<Option<InventoryLot>, RepositoryError> {
        debug!("Fetching inventory lot");
        let row = sqlx::query_as!(
            InventoryLotRow,
            r#"
            SELECT
                id, batch_id, lot_number, quantity,
                received_date as "received_date: NaiveDate",
                shopify_location_id, notes,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.inventory_lot
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?;

        if row.is_none() {
            debug!("Inventory lot not found");
        }
        Ok(row.map(Into::into))
    }

    /// Get an inventory lot with remaining quantity.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug", fields(lot_id = %id.as_i32()))]
    pub async fn get_lot_with_remaining(
        &self,
        id: InventoryLotId,
    ) -> Result<Option<InventoryLotWithRemaining>, RepositoryError> {
        debug!("Fetching inventory lot with remaining quantity");
        let row = sqlx::query_as!(
            InventoryLotWithRemainingRow,
            r#"
            SELECT
                l.id, l.batch_id, l.lot_number, l.quantity,
                l.received_date as "received_date: NaiveDate",
                l.shopify_location_id, l.notes,
                l.created_at as "created_at: DateTime<Utc>",
                l.updated_at as "updated_at: DateTime<Utc>",
                (l.quantity - COALESCE(SUM(a.quantity), 0))::bigint as "quantity_remaining!"
            FROM admin.inventory_lot l
            LEFT JOIN admin.lot_allocation a ON a.lot_id = l.id
            WHERE l.id = $1
            GROUP BY l.id
            "#,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?;

        if row.is_none() {
            debug!("Inventory lot not found");
        }
        Ok(row.map(Into::into))
    }

    /// List inventory lots for a batch with remaining quantities.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug", fields(batch_id = %batch_id.as_i32()))]
    pub async fn list_lots_for_batch(
        &self,
        batch_id: ManufacturingBatchId,
    ) -> Result<Vec<InventoryLotWithRemaining>, RepositoryError> {
        debug!("Listing inventory lots for batch");
        let rows = sqlx::query_as!(
            InventoryLotWithRemainingRow,
            r#"
            SELECT
                l.id, l.batch_id, l.lot_number, l.quantity,
                l.received_date as "received_date: NaiveDate",
                l.shopify_location_id, l.notes,
                l.created_at as "created_at: DateTime<Utc>",
                l.updated_at as "updated_at: DateTime<Utc>",
                (l.quantity - COALESCE(SUM(a.quantity), 0))::bigint as "quantity_remaining!"
            FROM admin.inventory_lot l
            LEFT JOIN admin.lot_allocation a ON a.lot_id = l.id
            WHERE l.batch_id = $1
            GROUP BY l.id
            ORDER BY l.received_date ASC, l.created_at ASC
            "#,
            batch_id.as_i32()
        )
        .fetch_all(self.pool)
        .await?;

        debug!(count = rows.len(), "Found inventory lots for batch");
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// List inventory lots with filtering.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, filter), level = "debug")]
    pub async fn list_lots(
        &self,
        filter: &LotFilter,
    ) -> Result<Vec<InventoryLotWithRemaining>, RepositoryError> {
        debug!("Listing inventory lots with filter");
        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);
        let batch_id = filter.batch_id.map(|id| id.as_i32());

        let rows = sqlx::query_as!(
            InventoryLotWithRemainingRow,
            r#"
            SELECT
                l.id, l.batch_id, l.lot_number, l.quantity,
                l.received_date as "received_date: NaiveDate",
                l.shopify_location_id, l.notes,
                l.created_at as "created_at: DateTime<Utc>",
                l.updated_at as "updated_at: DateTime<Utc>",
                (l.quantity - COALESCE(SUM(a.quantity), 0))::bigint as "quantity_remaining!"
            FROM admin.inventory_lot l
            LEFT JOIN admin.lot_allocation a ON a.lot_id = l.id
            LEFT JOIN admin.manufacturing_batch b ON b.id = l.batch_id
            WHERE
                ($1::int IS NULL OR l.batch_id = $1)
                AND ($2::text IS NULL OR b.shopify_product_id = $2)
                AND ($3::text IS NULL OR l.shopify_location_id = $3)
                AND ($4::date IS NULL OR l.received_date >= $4)
                AND ($5::date IS NULL OR l.received_date <= $5)
            GROUP BY l.id
            HAVING ($6::bool IS NULL OR NOT $6 OR (l.quantity - COALESCE(SUM(a.quantity), 0)) > 0)
            ORDER BY l.received_date ASC, l.created_at ASC
            LIMIT $7 OFFSET $8
            "#,
            batch_id,
            filter.shopify_product_id,
            filter.shopify_location_id,
            filter.start_date.map(to_time_date),
            filter.end_date.map(to_time_date),
            filter.has_remaining,
            limit,
            offset
        )
        .fetch_all(self.pool)
        .await?;

        debug!(count = rows.len(), "Found inventory lots");
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Get an inventory lot with batch info by ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get_lot_with_batch_info(
        &self,
        id: InventoryLotId,
    ) -> Result<Option<InventoryLotWithBatch>, RepositoryError> {
        let row = sqlx::query_as!(
            InventoryLotWithBatchRow,
            r#"
            SELECT
                l.id, l.batch_id, l.lot_number, l.quantity,
                l.received_date as "received_date: NaiveDate",
                l.shopify_location_id, l.notes,
                l.created_at as "created_at: DateTime<Utc>",
                l.updated_at as "updated_at: DateTime<Utc>",
                (l.quantity - COALESCE(SUM(a.quantity), 0))::bigint as "quantity_remaining!",
                b.batch_number,
                b.cost_per_unit as "cost_per_unit!: Decimal",
                b.currency_code
            FROM admin.inventory_lot l
            INNER JOIN admin.manufacturing_batch b ON b.id = l.batch_id
            LEFT JOIN admin.lot_allocation a ON a.lot_id = l.id
            WHERE l.id = $1
            GROUP BY l.id, b.batch_number, b.cost_per_unit, b.currency_code
            "#,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    /// Get available lots for a product with batch info (for FIFO allocation).
    ///
    /// Returns lots ordered by `received_date` ASC (oldest first) that have
    /// remaining quantity > 0.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get_available_lots_for_product(
        &self,
        shopify_product_id: &str,
    ) -> Result<Vec<InventoryLotWithBatch>, RepositoryError> {
        let rows = sqlx::query_as!(
            InventoryLotWithBatchRow,
            r#"
            SELECT
                l.id, l.batch_id, l.lot_number, l.quantity,
                l.received_date as "received_date: NaiveDate",
                l.shopify_location_id, l.notes,
                l.created_at as "created_at: DateTime<Utc>",
                l.updated_at as "updated_at: DateTime<Utc>",
                (l.quantity - COALESCE(SUM(a.quantity), 0))::bigint as "quantity_remaining!",
                b.batch_number,
                b.cost_per_unit as "cost_per_unit!: Decimal",
                b.currency_code
            FROM admin.inventory_lot l
            INNER JOIN admin.manufacturing_batch b ON b.id = l.batch_id
            LEFT JOIN admin.lot_allocation a ON a.lot_id = l.id
            WHERE b.shopify_product_id = $1
            GROUP BY l.id, b.batch_number, b.cost_per_unit, b.currency_code
            HAVING (l.quantity - COALESCE(SUM(a.quantity), 0)) > 0
            ORDER BY l.received_date ASC, l.created_at ASC
            "#,
            shopify_product_id
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Update an inventory lot.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the lot doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    #[instrument(skip(self, input), level = "debug", fields(lot_id = %id.as_i32()))]
    pub async fn update_lot(
        &self,
        id: InventoryLotId,
        input: &UpdateLotInput,
    ) -> Result<InventoryLot, RepositoryError> {
        debug!("Updating inventory lot");
        let row = sqlx::query_as!(
            InventoryLotRow,
            r#"
            UPDATE admin.inventory_lot
            SET
                lot_number = COALESCE($2, lot_number),
                quantity = COALESCE($3, quantity),
                received_date = COALESCE($4, received_date),
                shopify_location_id = COALESCE($5, shopify_location_id),
                notes = COALESCE($6, notes)
            WHERE id = $1
            RETURNING
                id, batch_id, lot_number, quantity,
                received_date as "received_date: NaiveDate",
                shopify_location_id, notes,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            "#,
            id.as_i32(),
            input.lot_number,
            input.quantity,
            input.received_date.map(to_time_date),
            input.shopify_location_id,
            input.notes
        )
        .fetch_optional(self.pool)
        .await?;

        let lot = row.ok_or_else(|| {
            debug!("Inventory lot not found for update");
            RepositoryError::NotFound
        })?;
        info!(lot_id = %id.as_i32(), "Inventory lot updated");
        Ok(lot.into())
    }

    /// Delete an inventory lot.
    ///
    /// Note: This will fail if there are allocations referencing this lot
    /// (due to RESTRICT foreign key).
    ///
    /// # Returns
    ///
    /// Returns `true` if the lot was deleted, `false` if it didn't exist.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug", fields(lot_id = %id.as_i32()))]
    pub async fn delete_lot(&self, id: InventoryLotId) -> Result<bool, RepositoryError> {
        debug!("Deleting inventory lot");
        let result = sqlx::query!(
            r#"
            DELETE FROM admin.inventory_lot
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!(lot_id = %id.as_i32(), "Inventory lot deleted");
        } else {
            debug!("Inventory lot not found for deletion");
        }
        Ok(deleted)
    }

    // =========================================================================
    // Allocation CRUD
    // =========================================================================

    /// Allocate units from a lot to an order line item.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Conflict` if the line item is already
    /// allocated to this lot.
    /// Returns `RepositoryError::Database` for other database errors.
    #[instrument(skip(self, input, allocated_by), level = "debug", fields(lot_id = %input.lot_id.as_i32(), quantity = input.quantity))]
    pub async fn allocate(
        &self,
        input: &AllocateLotInput,
        allocated_by: Option<AdminUserId>,
    ) -> Result<LotAllocation, RepositoryError> {
        debug!(order_id = %input.shopify_order_id, "Allocating units from lot");
        let row = sqlx::query_as!(
            LotAllocationRow,
            r#"
            INSERT INTO admin.lot_allocation (
                lot_id, shopify_order_id, shopify_line_item_id,
                quantity, allocated_by
            )
            VALUES ($1, $2, $3, $4, $5)
            RETURNING
                id, lot_id, shopify_order_id, shopify_line_item_id,
                quantity,
                allocated_at as "allocated_at: DateTime<Utc>",
                allocated_by
            "#,
            input.lot_id.as_i32(),
            input.shopify_order_id,
            input.shopify_line_item_id,
            input.quantity,
            allocated_by.map(|id| id.as_i32())
        )
        .fetch_one(self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e
                && db_err.constraint() == Some("idx_lot_allocation_line_item_lot")
            {
                warn!(lot_id = %input.lot_id.as_i32(), "Line item already allocated to this lot");
                return RepositoryError::Conflict(
                    "Line item already allocated to this lot".to_string(),
                );
            }
            RepositoryError::Database(e)
        })?;

        let allocation: LotAllocation = row.into();
        info!(allocation_id = %allocation.id.as_i32(), lot_id = %input.lot_id.as_i32(), quantity = input.quantity, "Lot allocation created");
        Ok(allocation)
    }

    /// Get allocations for a lot.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug", fields(lot_id = %lot_id.as_i32()))]
    pub async fn get_allocations_for_lot(
        &self,
        lot_id: InventoryLotId,
    ) -> Result<Vec<LotAllocation>, RepositoryError> {
        debug!("Fetching allocations for lot");
        let rows = sqlx::query_as!(
            LotAllocationRow,
            r#"
            SELECT
                id, lot_id, shopify_order_id, shopify_line_item_id,
                quantity,
                allocated_at as "allocated_at: DateTime<Utc>",
                allocated_by
            FROM admin.lot_allocation
            WHERE lot_id = $1
            ORDER BY allocated_at DESC
            "#,
            lot_id.as_i32()
        )
        .fetch_all(self.pool)
        .await?;

        debug!(count = rows.len(), "Found allocations for lot");
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Get allocations for an order.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug", fields(order_id = %shopify_order_id))]
    pub async fn get_allocations_for_order(
        &self,
        shopify_order_id: &str,
    ) -> Result<Vec<LotAllocation>, RepositoryError> {
        debug!("Fetching allocations for order");
        let rows = sqlx::query_as!(
            LotAllocationRow,
            r#"
            SELECT
                id, lot_id, shopify_order_id, shopify_line_item_id,
                quantity,
                allocated_at as "allocated_at: DateTime<Utc>",
                allocated_by
            FROM admin.lot_allocation
            WHERE shopify_order_id = $1
            ORDER BY allocated_at ASC
            "#,
            shopify_order_id
        )
        .fetch_all(self.pool)
        .await?;

        debug!(count = rows.len(), "Found allocations for order");
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Delete an allocation.
    ///
    /// # Returns
    ///
    /// Returns `true` if the allocation was deleted, `false` if it didn't exist.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug", fields(allocation_id = %id.as_i32()))]
    pub async fn delete_allocation(&self, id: LotAllocationId) -> Result<bool, RepositoryError> {
        debug!("Deleting lot allocation");
        let result = sqlx::query!(
            r#"
            DELETE FROM admin.lot_allocation
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!(allocation_id = %id.as_i32(), "Lot allocation deleted");
        } else {
            debug!("Lot allocation not found for deletion");
        }
        Ok(deleted)
    }

    /// Auto-allocate a line item to lots using FIFO.
    ///
    /// Allocates from the oldest available lot(s) until the requested quantity
    /// is fulfilled. May create multiple allocations if one lot doesn't have
    /// enough remaining quantity.
    ///
    /// # Returns
    ///
    /// Returns the allocations created, or empty vec if no lots available.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, allocated_by), level = "debug", fields(product_id = %shopify_product_id, order_id = %shopify_order_id, quantity = quantity_needed))]
    pub async fn auto_allocate_fifo(
        &self,
        shopify_product_id: &str,
        shopify_order_id: &str,
        shopify_line_item_id: &str,
        quantity_needed: i32,
        allocated_by: Option<AdminUserId>,
    ) -> Result<Vec<LotAllocation>, RepositoryError> {
        debug!("Auto-allocating line item using FIFO");
        let available_lots = self
            .get_available_lots_for_product(shopify_product_id)
            .await?;

        debug!(
            available_lots = available_lots.len(),
            "Found available lots for FIFO allocation"
        );

        let mut allocations = Vec::new();
        let mut remaining = quantity_needed;

        for lot in available_lots {
            if remaining <= 0 {
                break;
            }

            let qty_remaining_i32 = lot.quantity_remaining.try_into().unwrap_or(i32::MAX);
            let allocate_qty = remaining.min(qty_remaining_i32);

            let input = AllocateLotInput {
                lot_id: lot.lot.id,
                shopify_order_id: shopify_order_id.to_string(),
                shopify_line_item_id: shopify_line_item_id.to_string(),
                quantity: allocate_qty,
            };

            match self.allocate(&input, allocated_by).await {
                Ok(allocation) => {
                    remaining -= allocate_qty;
                    allocations.push(allocation);
                }
                Err(RepositoryError::Conflict(_)) => {
                    debug!(lot_id = %lot.lot.id.as_i32(), "Skipping lot - already allocated");
                }
                Err(e) => return Err(e),
            }
        }

        let allocated_quantity = quantity_needed - remaining;
        info!(
            allocations_created = allocations.len(),
            quantity_allocated = allocated_quantity,
            quantity_unfulfilled = remaining,
            "FIFO allocation completed"
        );
        Ok(allocations)
    }

    // =========================================================================
    // COGS Aggregation (for profit margin reporting)
    // =========================================================================

    /// Get COGS aggregated by product for a date range.
    ///
    /// Uses allocation dates to determine which COGS fall within the range.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_cogs_by_product(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<ProductCogs>, RepositoryError> {
        debug!("Aggregating COGS by product");

        let start_ts = to_time_offset_midnight(start);
        let end_exclusive = to_time_offset_next_midnight(end);

        let rows = sqlx::query!(
            r#"
            SELECT
                mb.shopify_product_id,
                COALESCE(SUM(la.quantity::numeric * mb.cost_per_unit), 0) as "total_cogs!: Decimal",
                COALESCE(SUM(la.quantity), 0)::bigint as "units_allocated!"
            FROM admin.lot_allocation la
            JOIN admin.inventory_lot il ON la.lot_id = il.id
            JOIN admin.manufacturing_batch mb ON il.batch_id = mb.id
            WHERE la.allocated_at >= $1
              AND la.allocated_at < $2
            GROUP BY mb.shopify_product_id
            "#,
            start_ts,
            end_exclusive,
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ProductCogs {
                shopify_product_id: r.shopify_product_id,
                total_cogs: r.total_cogs,
                units_allocated: r.units_allocated,
            })
            .collect())
    }

    /// Get COGS aggregated by order for a date range.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_cogs_by_order(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<OrderCogs>, RepositoryError> {
        debug!("Aggregating COGS by order");

        let start_ts = to_time_offset_midnight(start);
        let end_exclusive = to_time_offset_next_midnight(end);

        let rows = sqlx::query!(
            r#"
            SELECT
                la.shopify_order_id,
                COALESCE(SUM(la.quantity::numeric * mb.cost_per_unit), 0) as "total_cogs!: Decimal"
            FROM admin.lot_allocation la
            JOIN admin.inventory_lot il ON la.lot_id = il.id
            JOIN admin.manufacturing_batch mb ON il.batch_id = mb.id
            WHERE la.allocated_at >= $1
              AND la.allocated_at < $2
            GROUP BY la.shopify_order_id
            "#,
            start_ts,
            end_exclusive,
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| OrderCogs {
                shopify_order_id: r.shopify_order_id,
                total_cogs: r.total_cogs,
            })
            .collect())
    }

    /// Get total COGS for a date range.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_total_cogs(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Decimal, RepositoryError> {
        debug!("Calculating total COGS");

        let start_ts = to_time_offset_midnight(start);
        let end_exclusive = to_time_offset_next_midnight(end);

        let total = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(SUM(la.quantity::numeric * mb.cost_per_unit), 0) as "total!: Decimal"
            FROM admin.lot_allocation la
            JOIN admin.inventory_lot il ON la.lot_id = il.id
            JOIN admin.manufacturing_batch mb ON il.batch_id = mb.id
            WHERE la.allocated_at >= $1
              AND la.allocated_at < $2
            "#,
            start_ts,
            end_exclusive,
        )
        .fetch_one(self.pool)
        .await?;

        Ok(total)
    }
}

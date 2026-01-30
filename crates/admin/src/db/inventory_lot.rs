//! Database operations for inventory lots and allocations.
//!
//! All queries use sqlx macros for compile-time verification.

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use naked_pineapple_core::{AdminUserId, InventoryLotId, LotAllocationId, ManufacturingBatchId};

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
    pub async fn create_lot(
        &self,
        input: &CreateLotInput,
    ) -> Result<InventoryLot, RepositoryError> {
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

        Ok(row.into())
    }

    /// Get an inventory lot by ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get_lot(
        &self,
        id: InventoryLotId,
    ) -> Result<Option<InventoryLot>, RepositoryError> {
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

        Ok(row.map(Into::into))
    }

    /// Get an inventory lot with remaining quantity.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get_lot_with_remaining(
        &self,
        id: InventoryLotId,
    ) -> Result<Option<InventoryLotWithRemaining>, RepositoryError> {
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

        Ok(row.map(Into::into))
    }

    /// List inventory lots for a batch with remaining quantities.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn list_lots_for_batch(
        &self,
        batch_id: ManufacturingBatchId,
    ) -> Result<Vec<InventoryLotWithRemaining>, RepositoryError> {
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

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// List inventory lots with filtering.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn list_lots(
        &self,
        filter: &LotFilter,
    ) -> Result<Vec<InventoryLotWithRemaining>, RepositoryError> {
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
    pub async fn update_lot(
        &self,
        id: InventoryLotId,
        input: &UpdateLotInput,
    ) -> Result<InventoryLot, RepositoryError> {
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
        .await?
        .ok_or(RepositoryError::NotFound)?;

        Ok(row.into())
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
    pub async fn delete_lot(&self, id: InventoryLotId) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            r#"
            DELETE FROM admin.inventory_lot
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
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
    pub async fn allocate(
        &self,
        input: &AllocateLotInput,
        allocated_by: Option<AdminUserId>,
    ) -> Result<LotAllocation, RepositoryError> {
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
                return RepositoryError::Conflict(
                    "Line item already allocated to this lot".to_string(),
                );
            }
            RepositoryError::Database(e)
        })?;

        Ok(row.into())
    }

    /// Get allocations for a lot.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get_allocations_for_lot(
        &self,
        lot_id: InventoryLotId,
    ) -> Result<Vec<LotAllocation>, RepositoryError> {
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

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Get allocations for an order.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get_allocations_for_order(
        &self,
        shopify_order_id: &str,
    ) -> Result<Vec<LotAllocation>, RepositoryError> {
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
    pub async fn delete_allocation(&self, id: LotAllocationId) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            r#"
            DELETE FROM admin.lot_allocation
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
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
    pub async fn auto_allocate_fifo(
        &self,
        shopify_product_id: &str,
        shopify_order_id: &str,
        shopify_line_item_id: &str,
        quantity_needed: i32,
        allocated_by: Option<AdminUserId>,
    ) -> Result<Vec<LotAllocation>, RepositoryError> {
        let available_lots = self
            .get_available_lots_for_product(shopify_product_id)
            .await?;

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
                    // Already allocated to this lot, skip
                }
                Err(e) => return Err(e),
            }
        }

        Ok(allocations)
    }
}

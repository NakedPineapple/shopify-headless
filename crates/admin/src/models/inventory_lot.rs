//! Inventory lot domain models for tracking units received from manufacturing batches.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use naked_pineapple_core::{AdminUserId, InventoryLotId, LotAllocationId, ManufacturingBatchId};

/// An inventory lot - units received from a manufacturing batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryLot {
    /// Unique lot ID.
    pub id: InventoryLotId,
    /// Manufacturing batch this lot came from.
    pub batch_id: ManufacturingBatchId,
    /// Lot number identifier.
    pub lot_number: String,
    /// Number of units received.
    pub quantity: i32,
    /// Date received into inventory.
    pub received_date: NaiveDate,
    /// Optional Shopify location GID.
    pub shopify_location_id: Option<String>,
    /// Optional notes.
    pub notes: Option<String>,
    /// When the lot was created.
    pub created_at: DateTime<Utc>,
    /// When the lot was last updated.
    pub updated_at: DateTime<Utc>,
}

/// An inventory lot with computed remaining quantity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryLotWithRemaining {
    /// The lot itself.
    pub lot: InventoryLot,
    /// Quantity remaining after allocations.
    pub quantity_remaining: i64,
}

/// An inventory lot with parent batch info for cost lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryLotWithBatch {
    /// The lot itself.
    pub lot: InventoryLot,
    /// Quantity remaining after allocations.
    pub quantity_remaining: i64,
    /// Batch number from parent batch.
    pub batch_number: String,
    /// Cost per unit from parent batch.
    pub cost_per_unit: Decimal,
    /// Currency code from parent batch.
    pub currency_code: String,
}

/// An allocation of inventory lot units to an order line item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LotAllocation {
    /// Unique allocation ID.
    pub id: LotAllocationId,
    /// Lot allocated from.
    pub lot_id: InventoryLotId,
    /// Shopify order GID.
    pub shopify_order_id: String,
    /// Shopify line item GID.
    pub shopify_line_item_id: String,
    /// Number of units allocated.
    pub quantity: i32,
    /// When the allocation was made.
    pub allocated_at: DateTime<Utc>,
    /// Admin who made the allocation (if manual).
    pub allocated_by: Option<AdminUserId>,
}

/// An allocation with additional context for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LotAllocationWithContext {
    /// The allocation itself.
    pub allocation: LotAllocation,
    /// Order name from Shopify (e.g., "#1001").
    pub order_name: Option<String>,
}

/// Input for creating a new inventory lot.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateLotInput {
    /// Manufacturing batch this lot came from.
    pub batch_id: ManufacturingBatchId,
    /// Lot number identifier.
    pub lot_number: String,
    /// Number of units received.
    pub quantity: i32,
    /// Date received into inventory.
    pub received_date: NaiveDate,
    /// Optional Shopify location GID.
    pub shopify_location_id: Option<String>,
    /// Optional notes.
    pub notes: Option<String>,
}

/// Input for updating an inventory lot.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateLotInput {
    /// Lot number identifier.
    pub lot_number: Option<String>,
    /// Number of units received.
    pub quantity: Option<i32>,
    /// Date received into inventory.
    pub received_date: Option<NaiveDate>,
    /// Optional Shopify location GID.
    pub shopify_location_id: Option<String>,
    /// Optional notes.
    pub notes: Option<String>,
}

/// Input for allocating lot units to an order.
#[derive(Debug, Clone, Deserialize)]
pub struct AllocateLotInput {
    /// Lot to allocate from.
    pub lot_id: InventoryLotId,
    /// Shopify order GID.
    pub shopify_order_id: String,
    /// Shopify line item GID.
    pub shopify_line_item_id: String,
    /// Number of units to allocate.
    pub quantity: i32,
}

/// Filter criteria for listing lots.
#[derive(Debug, Clone, Default)]
pub struct LotFilter {
    /// Filter by batch ID.
    pub batch_id: Option<ManufacturingBatchId>,
    /// Filter by Shopify product ID (via batch).
    pub shopify_product_id: Option<String>,
    /// Filter by Shopify location ID.
    pub shopify_location_id: Option<String>,
    /// Filter by start date (inclusive).
    pub start_date: Option<NaiveDate>,
    /// Filter by end date (inclusive).
    pub end_date: Option<NaiveDate>,
    /// Only lots with remaining quantity > 0.
    pub has_remaining: Option<bool>,
    /// Maximum number of results.
    pub limit: Option<i64>,
    /// Number of results to skip.
    pub offset: Option<i64>,
}

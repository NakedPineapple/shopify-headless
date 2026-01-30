//! Manufacturing batch domain models for cost tracking.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use naked_pineapple_core::{BatchMetadataId, ManufacturingBatchId};

/// A manufacturing batch/production run with cost tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManufacturingBatch {
    /// Unique batch ID.
    pub id: ManufacturingBatchId,
    /// Batch/lot number identifier.
    pub batch_number: String,
    /// Shopify product GID.
    pub shopify_product_id: String,
    /// Optional Shopify variant GID.
    pub shopify_variant_id: Option<String>,
    /// Number of units in this production run.
    pub quantity: i32,
    /// Date of manufacture.
    pub manufacture_date: NaiveDate,
    /// Raw material cost per item.
    pub raw_cost_per_item: Decimal,
    /// Label cost per item.
    pub label_cost_per_item: Decimal,
    /// Outer carton cost per item.
    pub outer_carton_cost_per_item: Decimal,
    /// Computed cost per unit (raw + label + carton).
    pub cost_per_unit: Decimal,
    /// Computed total batch cost (`cost_per_unit` * quantity).
    pub total_batch_cost: Decimal,
    /// Currency code (ISO 4217).
    pub currency_code: String,
    /// Optional notes.
    pub notes: Option<String>,
    /// When the batch was created.
    pub created_at: DateTime<Utc>,
    /// When the batch was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Key-value metadata for a manufacturing batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMetadata {
    /// Unique metadata ID.
    pub id: BatchMetadataId,
    /// Batch this metadata belongs to.
    pub batch_id: ManufacturingBatchId,
    /// Metadata key.
    pub key: String,
    /// Metadata value (JSON).
    pub value: serde_json::Value,
    /// When the metadata was created.
    pub created_at: DateTime<Utc>,
}

/// A manufacturing batch with all related data loaded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManufacturingBatchWithDetails {
    /// The batch itself.
    pub batch: ManufacturingBatch,
    /// Metadata key-value pairs.
    pub metadata: Vec<BatchMetadata>,
    /// Product title from Shopify (loaded at display time).
    pub product_title: Option<String>,
    /// Total units received as lots from this batch.
    pub lots_received: i64,
}

/// Input for creating a new manufacturing batch.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateBatchInput {
    /// Batch/lot number identifier.
    pub batch_number: String,
    /// Shopify product GID.
    pub shopify_product_id: String,
    /// Optional Shopify variant GID.
    pub shopify_variant_id: Option<String>,
    /// Number of units in this production run.
    pub quantity: i32,
    /// Date of manufacture.
    pub manufacture_date: NaiveDate,
    /// Raw material cost per item.
    pub raw_cost_per_item: Decimal,
    /// Label cost per item.
    pub label_cost_per_item: Decimal,
    /// Outer carton cost per item.
    pub outer_carton_cost_per_item: Decimal,
    /// Currency code (ISO 4217).
    pub currency_code: String,
    /// Optional notes.
    pub notes: Option<String>,
}

/// Input for updating a manufacturing batch.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateBatchInput {
    /// Batch/lot number identifier.
    pub batch_number: Option<String>,
    /// Number of units in this production run.
    pub quantity: Option<i32>,
    /// Date of manufacture.
    pub manufacture_date: Option<NaiveDate>,
    /// Raw material cost per item.
    pub raw_cost_per_item: Option<Decimal>,
    /// Label cost per item.
    pub label_cost_per_item: Option<Decimal>,
    /// Outer carton cost per item.
    pub outer_carton_cost_per_item: Option<Decimal>,
    /// Currency code (ISO 4217).
    pub currency_code: Option<String>,
    /// Optional notes.
    pub notes: Option<String>,
}

/// Filter criteria for listing batches.
#[derive(Debug, Clone, Default)]
pub struct BatchFilter {
    /// Filter by Shopify product ID.
    pub shopify_product_id: Option<String>,
    /// Filter by start date (inclusive).
    pub start_date: Option<NaiveDate>,
    /// Filter by end date (inclusive).
    pub end_date: Option<NaiveDate>,
    /// Search by batch number.
    pub batch_number: Option<String>,
    /// Maximum number of results.
    pub limit: Option<i64>,
    /// Number of results to skip.
    pub offset: Option<i64>,
}

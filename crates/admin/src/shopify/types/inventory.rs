//! Location and inventory domain types for Shopify Admin API.

use serde::{Deserialize, Serialize};

use super::common::{Image, Money, PageInfo};
use super::product::ProductStatus;

// =============================================================================
// Location Types
// =============================================================================

/// A physical location for inventory storage and fulfillment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    /// Location ID.
    pub id: String,
    /// Location name.
    pub name: String,
    /// Whether the location is active.
    pub is_active: bool,
    /// Whether this location fulfills online orders.
    pub fulfills_online_orders: bool,
    /// Location address.
    pub address: Option<LocationAddress>,
}

/// Simplified address for a location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationAddress {
    /// Street address.
    pub address1: Option<String>,
    /// City.
    pub city: Option<String>,
    /// Province/state code.
    pub province_code: Option<String>,
    /// Country code.
    pub country_code: Option<String>,
    /// Postal/ZIP code.
    pub zip: Option<String>,
}

/// Paginated list of locations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationConnection {
    /// Locations in this page.
    pub locations: Vec<Location>,
    /// Pagination info.
    pub page_info: PageInfo,
}

// =============================================================================
// Inventory Types
// =============================================================================

/// Inventory level at a specific location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryLevel {
    /// Inventory item ID.
    pub inventory_item_id: String,
    /// Location ID.
    pub location_id: String,
    /// Location name.
    pub location_name: Option<String>,
    /// Quantity available.
    pub available: i64,
    /// Quantity on hand.
    pub on_hand: i64,
    /// Quantity incoming.
    pub incoming: i64,
    /// Last update timestamp.
    pub updated_at: Option<String>,
}

/// Result of an inventory adjustment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryAdjustmentResult {
    /// The affected inventory level.
    pub inventory_level: InventoryLevel,
}

/// An inventory item with full details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItem {
    /// Inventory item ID.
    pub id: String,
    /// SKU code.
    pub sku: Option<String>,
    /// Whether inventory is tracked.
    pub tracked: bool,
    /// Whether the item requires shipping.
    pub requires_shipping: bool,
    /// Unit cost.
    pub unit_cost: Option<Money>,
    /// Harmonized System (HS) code for customs.
    pub harmonized_system_code: Option<String>,
    /// Country of origin (ISO 3166-1 alpha-2).
    pub country_code_of_origin: Option<String>,
    /// Province of origin.
    pub province_code_of_origin: Option<String>,
    /// Inventory levels at different locations.
    pub inventory_levels: Vec<InventoryLevel>,
    /// Associated variant.
    pub variant: Option<InventoryItemVariant>,
}

/// Variant info associated with an inventory item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItemVariant {
    /// Variant ID.
    pub id: String,
    /// Variant title.
    pub title: String,
    /// Display name.
    pub display_name: Option<String>,
    /// Price.
    pub price: Option<String>,
    /// Variant image.
    pub image: Option<Image>,
    /// Associated product.
    pub product: Option<InventoryItemProduct>,
}

/// Product info associated with an inventory item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItemProduct {
    /// Product ID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// URL handle.
    pub handle: String,
    /// Product status.
    pub status: ProductStatus,
    /// Featured image.
    pub featured_image: Option<Image>,
}

/// Paginated list of inventory items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItemConnection {
    /// Inventory items in this page.
    pub items: Vec<InventoryItem>,
    /// Pagination info.
    pub page_info: PageInfo,
}

/// Inventory adjustment reason codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InventoryAdjustmentReason {
    /// Correction to inventory count.
    Correction,
    /// Cycle count verification.
    CycleCountAvailable,
    /// Items damaged.
    Damaged,
    /// Items received from supplier.
    Received,
    /// Items restocked (returned to inventory).
    Restock,
    /// Inventory shrinkage (loss/theft).
    Shrinkage,
    /// Other reason.
    Other,
}

impl std::fmt::Display for InventoryAdjustmentReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Correction => write!(f, "Correction"),
            Self::CycleCountAvailable => write!(f, "Cycle Count"),
            Self::Damaged => write!(f, "Damaged"),
            Self::Received => write!(f, "Received"),
            Self::Restock => write!(f, "Restock"),
            Self::Shrinkage => write!(f, "Shrinkage"),
            Self::Other => write!(f, "Other"),
        }
    }
}

/// Input for updating an inventory item.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InventoryItemUpdateInput {
    /// New SKU.
    pub sku: Option<String>,
    /// Whether to track inventory.
    pub tracked: Option<bool>,
    /// Whether item requires shipping.
    pub requires_shipping: Option<bool>,
    /// Unit cost amount.
    pub cost: Option<String>,
    /// Harmonized System code.
    pub harmonized_system_code: Option<String>,
    /// Country of origin.
    pub country_code_of_origin: Option<String>,
    /// Province of origin.
    pub province_code_of_origin: Option<String>,
}

/// Paginated list of inventory levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryLevelConnection {
    /// Inventory levels in this page.
    pub inventory_levels: Vec<InventoryLevel>,
    /// Pagination info.
    pub page_info: PageInfo,
}

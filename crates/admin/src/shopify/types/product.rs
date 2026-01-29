//! Product domain types for Shopify Admin API.

use serde::{Deserialize, Serialize};

use super::common::{Image, Money, PageInfo};

// =============================================================================
// Product Types
// =============================================================================

/// Product status in the admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProductStatus {
    /// Product is visible on the storefront.
    Active,
    /// Product is not visible (work in progress).
    Draft,
    /// Product is hidden/archived.
    Archived,
    /// Product is unlisted (not shown in search/collections but accessible via URL).
    Unlisted,
}

/// A product variant with admin-specific fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminProductVariant {
    /// Variant ID.
    pub id: String,
    /// Variant title (combination of option values).
    pub title: String,
    /// SKU code.
    pub sku: Option<String>,
    /// Barcode.
    pub barcode: Option<String>,
    /// Current price.
    pub price: Money,
    /// Compare-at price (original price if on sale).
    pub compare_at_price: Option<Money>,
    /// Inventory quantity (across all locations).
    pub inventory_quantity: i64,
    /// Inventory item ID (for inventory operations).
    pub inventory_item_id: String,
    /// Whether inventory is tracked.
    pub inventory_management: Option<String>,
    /// Weight value.
    pub weight: Option<f64>,
    /// Weight unit (KILOGRAMS, GRAMS, POUNDS, OUNCES).
    pub weight_unit: Option<String>,
    /// Whether requires shipping.
    pub requires_shipping: bool,
    /// Variant image.
    pub image: Option<Image>,
    /// Creation timestamp.
    pub created_at: Option<String>,
    /// Last update timestamp.
    pub updated_at: Option<String>,
}

/// A product in the admin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminProduct {
    /// Product ID.
    pub id: String,
    /// URL handle.
    pub handle: String,
    /// Product title.
    pub title: String,
    /// Plain text description.
    pub description: String,
    /// HTML description.
    pub description_html: String,
    /// Product status (Active, Draft, Archived).
    pub status: ProductStatus,
    /// Product type/category.
    #[serde(rename = "product_type")]
    pub kind: String,
    /// Vendor name.
    pub vendor: String,
    /// Product tags.
    pub tags: Vec<String>,
    /// Total inventory quantity across all variants.
    pub total_inventory: i64,
    /// Creation timestamp.
    pub created_at: Option<String>,
    /// Last update timestamp.
    pub updated_at: Option<String>,
    /// Featured image.
    pub featured_image: Option<Image>,
    /// All product images.
    pub images: Vec<Image>,
    /// Product variants.
    pub variants: Vec<AdminProductVariant>,
}

// =============================================================================
// Pagination Types
// =============================================================================

/// Paginated list of products.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminProductConnection {
    /// Products in this page.
    pub products: Vec<AdminProduct>,
    /// Pagination info.
    pub page_info: PageInfo,
}

// =============================================================================
// Sort Keys
// =============================================================================

/// Sort keys for product queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProductSortKey {
    /// Sort by title.
    Title,
    /// Sort by product type.
    ProductType,
    /// Sort by vendor.
    Vendor,
    /// Sort by inventory total.
    InventoryTotal,
    /// Sort by last update.
    UpdatedAt,
    /// Sort by creation date.
    CreatedAt,
    /// Sort by ID.
    Id,
}

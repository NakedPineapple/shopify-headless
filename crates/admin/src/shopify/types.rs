//! Domain types for Shopify Admin API.
//!
//! These types provide a clean, ergonomic API separate from the raw
//! `graphql_client` generated types.

use serde::{Deserialize, Serialize};

// =============================================================================
// Money Types
// =============================================================================

/// Monetary amount with currency code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    /// Decimal amount as string (preserves precision).
    pub amount: String,
    /// ISO 4217 currency code.
    pub currency_code: String,
}

// =============================================================================
// Image Types
// =============================================================================

/// Product or media image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    /// Shopify image ID.
    pub id: Option<String>,
    /// Image URL.
    pub url: String,
    /// Alt text for accessibility.
    pub alt_text: Option<String>,
    /// Image width in pixels.
    pub width: Option<i64>,
    /// Image height in pixels.
    pub height: Option<i64>,
}

// =============================================================================
// Address Types
// =============================================================================

/// Mailing address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    /// First line of the address.
    pub address1: Option<String>,
    /// Second line of the address.
    pub address2: Option<String>,
    /// City.
    pub city: Option<String>,
    /// Province or state code.
    pub province_code: Option<String>,
    /// Country code (ISO 3166-1 alpha-2).
    pub country_code: Option<String>,
    /// Postal/ZIP code.
    pub zip: Option<String>,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Company name.
    pub company: Option<String>,
    /// Phone number.
    pub phone: Option<String>,
}

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
// Order Types
// =============================================================================

/// Order financial status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinancialStatus {
    /// No payment authorized.
    Pending,
    /// Payment has been authorized but not captured.
    Authorized,
    /// Payment has been captured.
    Paid,
    /// Payment has been partially paid.
    PartiallyPaid,
    /// Payment has been refunded.
    Refunded,
    /// Payment has been partially refunded.
    PartiallyRefunded,
    /// Payment has been voided.
    Voided,
    /// Payment has expired.
    Expired,
}

/// Order fulfillment status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FulfillmentStatus {
    /// No items have been fulfilled.
    Unfulfilled,
    /// Some items have been fulfilled.
    PartiallyFulfilled,
    /// All items have been fulfilled.
    Fulfilled,
    /// Fulfillment is on hold.
    OnHold,
    /// Items are being prepared.
    InProgress,
    /// Order was restocked.
    Restocked,
    /// Scheduled for fulfillment.
    Scheduled,
    /// Pending fulfillment.
    PendingFulfillment,
    /// Order is open.
    Open,
    /// Fulfillment request was declined.
    RequestDeclined,
}

/// Tracking information for a fulfillment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingInfo {
    /// Tracking company/carrier name.
    pub company: Option<String>,
    /// Tracking number.
    pub number: Option<String>,
    /// Tracking URL.
    pub url: Option<String>,
}

/// A fulfillment for an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fulfillment {
    /// Fulfillment ID.
    pub id: String,
    /// Fulfillment status.
    pub status: String,
    /// Tracking information.
    pub tracking_info: Vec<TrackingInfo>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

/// A line item in an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderLineItem {
    /// Line item ID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// Variant title.
    pub variant_title: Option<String>,
    /// SKU.
    pub sku: Option<String>,
    /// Quantity ordered.
    pub quantity: i64,
    /// Price per unit.
    pub original_unit_price: Money,
    /// Discounted price per unit.
    pub discounted_unit_price: Money,
    /// Total discount amount.
    pub total_discount: Money,
    /// Product ID.
    pub product_id: Option<String>,
    /// Variant ID.
    pub variant_id: Option<String>,
    /// Whether requires shipping.
    pub requires_shipping: bool,
    /// Whether is a gift card.
    pub is_gift_card: bool,
}

/// An order in the admin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Order ID.
    pub id: String,
    /// Order name (e.g., "#1001").
    pub name: String,
    /// Order number.
    pub number: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
    /// Financial status.
    pub financial_status: Option<FinancialStatus>,
    /// Fulfillment status.
    pub fulfillment_status: Option<FulfillmentStatus>,
    /// Whether the order is fully paid.
    pub fully_paid: bool,
    /// Whether the order is test mode.
    pub test: bool,
    /// Customer email.
    pub email: Option<String>,
    /// Customer phone.
    pub phone: Option<String>,
    /// Order note.
    pub note: Option<String>,
    /// Subtotal price.
    pub subtotal_price: Money,
    /// Total shipping price.
    pub total_shipping_price: Money,
    /// Total tax.
    pub total_tax: Money,
    /// Total price.
    pub total_price: Money,
    /// Total discount amount.
    pub total_discounts: Money,
    /// Currency code.
    pub currency_code: String,
    /// Line items.
    pub line_items: Vec<OrderLineItem>,
    /// Fulfillments.
    pub fulfillments: Vec<Fulfillment>,
    /// Billing address.
    pub billing_address: Option<Address>,
    /// Shipping address.
    pub shipping_address: Option<Address>,
    /// Customer ID.
    pub customer_id: Option<String>,
}

// =============================================================================
// Customer Types
// =============================================================================

/// Customer account state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CustomerState {
    /// Customer has not yet accepted the invite.
    Disabled,
    /// Customer has accepted the invite.
    Enabled,
    /// Customer was invited but hasn't accepted.
    Invited,
    /// Customer account was declined.
    Declined,
}

/// A customer in the admin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    /// Customer ID.
    pub id: String,
    /// Email address.
    pub email: Option<String>,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Display name.
    pub display_name: String,
    /// Phone number.
    pub phone: Option<String>,
    /// Account state.
    pub state: CustomerState,
    /// Whether marketing is accepted.
    pub accepts_marketing: bool,
    /// Marketing opt-in level.
    pub accepts_marketing_updated_at: Option<String>,
    /// Total orders count.
    pub orders_count: i64,
    /// Total amount spent.
    pub total_spent: Money,
    /// Customer note.
    pub note: Option<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Default address.
    pub default_address: Option<Address>,
    /// All addresses.
    pub addresses: Vec<Address>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
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

// =============================================================================
// Pagination Types
// =============================================================================

/// Pagination information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    /// Whether there are more items after this page.
    pub has_next_page: bool,
    /// Whether there are items before this page.
    pub has_previous_page: bool,
    /// Cursor for the first item.
    pub start_cursor: Option<String>,
    /// Cursor for the last item.
    pub end_cursor: Option<String>,
}

/// Paginated list of products.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminProductConnection {
    /// Products in this page.
    pub products: Vec<AdminProduct>,
    /// Pagination info.
    pub page_info: PageInfo,
}

/// Paginated list of orders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderConnection {
    /// Orders in this page.
    pub orders: Vec<Order>,
    /// Pagination info.
    pub page_info: PageInfo,
}

/// Paginated list of customers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerConnection {
    /// Customers in this page.
    pub customers: Vec<Customer>,
    /// Pagination info.
    pub page_info: PageInfo,
}

/// Paginated list of inventory levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryLevelConnection {
    /// Inventory levels in this page.
    pub inventory_levels: Vec<InventoryLevel>,
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

/// Sort keys for order queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderSortKey {
    /// Sort by order number.
    OrderNumber,
    /// Sort by total price.
    TotalPrice,
    /// Sort by creation date.
    CreatedAt,
    /// Sort by last update.
    UpdatedAt,
    /// Sort by customer name.
    CustomerName,
    /// Sort by financial status.
    FinancialStatus,
    /// Sort by fulfillment status.
    FulfillmentStatus,
    /// Sort by ID.
    Id,
}

/// Sort keys for customer queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CustomerSortKey {
    /// Sort by name.
    Name,
    /// Sort by location.
    Location,
    /// Sort by orders count.
    OrdersCount,
    /// Sort by total spent.
    TotalSpent,
    /// Sort by last order date.
    LastOrderDate,
    /// Sort by creation date.
    CreatedAt,
    /// Sort by last update.
    UpdatedAt,
    /// Sort by ID.
    Id,
}

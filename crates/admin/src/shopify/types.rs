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
    /// Address ID (for mutations).
    pub id: Option<String>,
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

/// Input for creating/updating a mailing address.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddressInput {
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
// Metafield Types
// =============================================================================

/// A metafield for storing custom data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metafield {
    /// Metafield ID.
    pub id: Option<String>,
    /// Namespace for grouping metafields.
    pub namespace: String,
    /// Key within the namespace.
    pub key: String,
    /// The metafield value.
    pub value: String,
}

/// Input for creating/updating metafields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetafieldInput {
    /// Namespace for the metafield.
    pub namespace: String,
    /// Key within the namespace.
    pub key: String,
    /// The value to store.
    pub value: String,
    /// The metafield type (e.g., `single_line_text_field`, `number_integer`).
    pub type_: String,
}

/// Parameters for updating a customer.
#[derive(Debug, Clone, Default)]
pub struct CustomerUpdateParams {
    /// Email address.
    pub email: Option<String>,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Phone number.
    pub phone: Option<String>,
    /// Note about the customer.
    pub note: Option<String>,
    /// Tags to set on the customer.
    pub tags: Option<Vec<String>>,
}

/// Override settings for customer merge operation.
///
/// Each field indicates whether to take the value from the source customer
/// (being merged) instead of the target customer (that remains).
// Allow: Each boolean represents an independent merge override choice from
// the Shopify API with no logical grouping into enums or state machines.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default)]
pub struct CustomerMergeOverrides {
    /// Take first name from the source customer.
    pub first_name: bool,
    /// Take last name from the source customer.
    pub last_name: bool,
    /// Take email from the source customer.
    pub email: bool,
    /// Take phone from the source customer.
    pub phone: bool,
    /// Take default address from the source customer.
    pub default_address: bool,
}

/// Identifier for a metafield (used in delete operations).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetafieldIdentifier {
    /// The owner resource ID.
    pub owner_id: String,
    /// Namespace of the metafield.
    pub namespace: String,
    /// Key of the metafield.
    pub key: String,
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

/// Sort key for customer lists.
///
/// Some keys are supported natively by Shopify API, others require
/// client-side sorting in Rust after fetching data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CustomerSortKey {
    // === Shopify API supported ===
    /// Sort by creation date (Shopify native).
    CreatedAt,
    /// Sort by ID (Shopify native).
    Id,
    /// Sort by location (Shopify native).
    Location,
    /// Sort by name (Shopify native).
    #[default]
    Name,
    /// Sort by relevance for search queries (Shopify native).
    Relevance,
    /// Sort by last update date (Shopify native).
    UpdatedAt,

    // === Client-side sorting (Rust) ===
    /// Sort by total amount spent.
    AmountSpent,
    /// Sort by total orders count.
    OrdersCount,
}

impl CustomerSortKey {
    /// Parse a sort key from a URL parameter string.
    #[must_use]
    pub fn from_str_param(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "created_at" | "created" => Some(Self::CreatedAt),
            "id" => Some(Self::Id),
            "location" => Some(Self::Location),
            "name" => Some(Self::Name),
            "relevance" => Some(Self::Relevance),
            "updated_at" | "updated" => Some(Self::UpdatedAt),
            "amount_spent" | "spent" => Some(Self::AmountSpent),
            "orders_count" | "orders" => Some(Self::OrdersCount),
            _ => None,
        }
    }

    /// Get the URL parameter string for this sort key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CreatedAt => "created_at",
            Self::Id => "id",
            Self::Location => "location",
            Self::Name => "name",
            Self::Relevance => "relevance",
            Self::UpdatedAt => "updated_at",
            Self::AmountSpent => "amount_spent",
            Self::OrdersCount => "orders_count",
        }
    }

    /// Whether this sort key is supported natively by Shopify API.
    #[must_use]
    pub const fn is_shopify_native(self) -> bool {
        matches!(
            self,
            Self::CreatedAt
                | Self::Id
                | Self::Location
                | Self::Name
                | Self::Relevance
                | Self::UpdatedAt
        )
    }
}

/// Parameters for listing customers.
#[derive(Debug, Clone, Default)]
pub struct CustomerListParams {
    /// Maximum number of customers to return.
    pub first: Option<i64>,
    /// Cursor for pagination.
    pub after: Option<String>,
    /// Search/filter query string (Shopify query syntax).
    pub query: Option<String>,
    /// Sort key.
    pub sort_key: Option<CustomerSortKey>,
    /// Whether to reverse sort order.
    pub reverse: bool,
}

/// Marketing consent state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarketingState {
    /// Not subscribed to marketing.
    NotSubscribed,
    /// Pending confirmation.
    Pending,
    /// Subscribed to marketing.
    Subscribed,
    /// Unsubscribed from marketing.
    Unsubscribed,
    /// Data has been redacted.
    Redacted,
    /// Invalid state.
    Invalid,
}

/// Marketing consent information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketingConsent {
    /// Current marketing state.
    pub state: MarketingState,
    /// Opt-in level (e.g., `SINGLE_OPT_IN`, `CONFIRMED_OPT_IN`).
    pub opt_in_level: Option<String>,
    /// When consent was last updated.
    pub consent_updated_at: Option<String>,
}

/// A customer's recent order (for detail view).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerOrder {
    /// Order ID.
    pub id: String,
    /// Order name (e.g., "#1001").
    pub name: String,
    /// Creation timestamp.
    pub created_at: String,
    /// Financial status display string.
    pub financial_status: Option<String>,
    /// Fulfillment status display string.
    pub fulfillment_status: Option<String>,
    /// Total price.
    pub total_price: Money,
}

/// A customer in the admin.
// Allow: Shopify API Customer object has independent boolean properties
// (accepts_marketing, tax_exempt, can_delete, is_mergeable) that cannot be grouped.
#[allow(clippy::struct_excessive_bools)]
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
    /// Customer locale (language preference).
    pub locale: Option<String>,
    /// Whether email marketing is accepted (legacy field).
    pub accepts_marketing: bool,
    /// Marketing opt-in timestamp (legacy field).
    pub accepts_marketing_updated_at: Option<String>,
    /// Email marketing consent details.
    pub email_marketing_consent: Option<MarketingConsent>,
    /// SMS marketing consent details.
    pub sms_marketing_consent: Option<MarketingConsent>,
    /// Total orders count.
    pub orders_count: i64,
    /// Total amount spent.
    pub total_spent: Money,
    /// Human-readable lifetime duration (e.g., "2 years").
    pub lifetime_duration: Option<String>,
    /// Whether customer is tax exempt.
    pub tax_exempt: bool,
    /// List of tax exemption codes.
    pub tax_exemptions: Vec<String>,
    /// Customer note.
    pub note: Option<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Whether this customer can be deleted (no orders).
    pub can_delete: bool,
    /// Whether this customer can be merged with another.
    pub is_mergeable: bool,
    /// Default address.
    pub default_address: Option<Address>,
    /// All addresses.
    pub addresses: Vec<Address>,
    /// Recent orders (populated on detail view).
    pub recent_orders: Vec<CustomerOrder>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

// =============================================================================
// Collection Types
// =============================================================================

/// A rule that defines membership in a smart collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionRule {
    /// The attribute column to check (TAG, TITLE, VENDOR, `PRODUCT_TYPE`, etc.).
    pub column: String,
    /// The relation operator (EQUALS, `NOT_EQUALS`, CONTAINS, etc.).
    pub relation: String,
    /// The value to match against.
    pub condition: String,
}

/// A set of rules that define a smart collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionRuleSet {
    /// If true, products matching ANY rule are included (OR logic).
    /// If false, products must match ALL rules (AND logic).
    pub applied_disjunctively: bool,
    /// The individual rules in this set.
    pub rules: Vec<CollectionRule>,
}

/// SEO metadata for a collection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionSeo {
    /// SEO title (shown in search results).
    pub title: Option<String>,
    /// SEO meta description.
    pub description: Option<String>,
}

/// A sales channel/publication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Publication {
    /// Publication ID.
    pub id: String,
    /// Publication name (e.g., "Online Store", "TikTok").
    pub name: String,
}

/// Publication status for a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePublication {
    /// The publication/sales channel.
    pub publication: Publication,
    /// Whether the resource is published on this channel.
    pub is_published: bool,
}

/// A product collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    /// Collection ID.
    pub id: String,
    /// Collection title.
    pub title: String,
    /// URL handle.
    pub handle: String,
    /// Plain text description.
    pub description: String,
    /// HTML description.
    pub description_html: Option<String>,
    /// Number of products in the collection.
    pub products_count: i64,
    /// Collection image.
    pub image: Option<Image>,
    /// Last update timestamp.
    pub updated_at: Option<String>,
    /// Rule set for smart collections (None for manual collections).
    pub rule_set: Option<CollectionRuleSet>,
    /// Sort order for products in the collection.
    pub sort_order: Option<String>,
    /// SEO metadata.
    pub seo: Option<CollectionSeo>,
    /// Publication status on each sales channel.
    pub publications: Vec<ResourcePublication>,
}

/// A product within a collection (simplified view).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionProduct {
    /// Product ID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// URL handle.
    pub handle: String,
    /// Product status (ACTIVE, DRAFT, ARCHIVED).
    pub status: String,
    /// Featured image URL.
    pub image_url: Option<String>,
    /// Total inventory quantity.
    pub total_inventory: i64,
    /// Minimum variant price.
    pub price: String,
    /// Currency code.
    pub currency_code: String,
}

/// A collection with its products.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionWithProducts {
    /// The collection.
    pub collection: Collection,
    /// Products in this collection.
    pub products: Vec<CollectionProduct>,
    /// Whether there are more products to load.
    pub has_next_page: bool,
    /// Cursor for loading more products.
    pub end_cursor: Option<String>,
}

/// Paginated list of collections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionConnection {
    /// Collections in this page.
    pub collections: Vec<Collection>,
    /// Pagination info.
    pub page_info: PageInfo,
}

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderSortKey {
    /// Sort by order number.
    OrderNumber,
    /// Sort by total price.
    TotalPrice,
    /// Sort by creation date.
    #[default]
    CreatedAt,
    /// Sort by processed date.
    ProcessedAt,
    /// Sort by last update.
    UpdatedAt,
    /// Sort by customer name.
    CustomerName,
    /// Sort by financial status.
    FinancialStatus,
    /// Sort by fulfillment status.
    FulfillmentStatus,
    /// Sort by destination.
    Destination,
    /// Sort by ID.
    Id,
}

impl OrderSortKey {
    /// Parse a sort key from a URL parameter string.
    #[must_use]
    pub fn from_str_param(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "order_number" | "number" => Some(Self::OrderNumber),
            "total_price" | "total" => Some(Self::TotalPrice),
            "created_at" | "created" => Some(Self::CreatedAt),
            "processed_at" | "processed" => Some(Self::ProcessedAt),
            "updated_at" | "updated" => Some(Self::UpdatedAt),
            "customer_name" | "customer" => Some(Self::CustomerName),
            "financial_status" | "payment" => Some(Self::FinancialStatus),
            "fulfillment_status" | "fulfillment" => Some(Self::FulfillmentStatus),
            "destination" => Some(Self::Destination),
            "id" => Some(Self::Id),
            _ => None,
        }
    }

    /// Get the URL parameter string for this sort key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OrderNumber => "order_number",
            Self::TotalPrice => "total_price",
            Self::CreatedAt => "created_at",
            Self::ProcessedAt => "processed_at",
            Self::UpdatedAt => "updated_at",
            Self::CustomerName => "customer_name",
            Self::FinancialStatus => "financial_status",
            Self::FulfillmentStatus => "fulfillment_status",
            Self::Destination => "destination",
            Self::Id => "id",
        }
    }
}

/// Order risk level from fraud analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderRiskLevel {
    /// Low risk order.
    Low,
    /// Medium risk order.
    Medium,
    /// High risk order.
    High,
}

impl std::fmt::Display for OrderRiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High => write!(f, "High"),
        }
    }
}

/// Order return status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderReturnStatus {
    /// No return.
    NoReturn,
    /// Return requested by customer.
    ReturnRequested,
    /// Return in progress.
    InProgress,
    /// Return completed.
    Returned,
}

impl std::fmt::Display for OrderReturnStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoReturn => write!(f, "No Return"),
            Self::ReturnRequested => write!(f, "Return Requested"),
            Self::InProgress => write!(f, "In Progress"),
            Self::Returned => write!(f, "Returned"),
        }
    }
}

/// Delivery category for shipping lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DeliveryCategory {
    /// Standard shipping delivery.
    Shipping,
    /// Local delivery.
    LocalDelivery,
    /// Store pickup.
    Pickup,
    /// Digital delivery (no physical shipping).
    None,
}

impl std::fmt::Display for DeliveryCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shipping => write!(f, "Shipping"),
            Self::LocalDelivery => write!(f, "Local Delivery"),
            Self::Pickup => write!(f, "Pickup"),
            Self::None => write!(f, "Digital"),
        }
    }
}

/// Risk assessment for an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRisk {
    /// Risk level.
    pub level: OrderRiskLevel,
    /// Risk message/reason.
    pub message: Option<String>,
}

/// Shipping line information for an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderShippingLine {
    /// Shipping method title.
    pub title: String,
    /// Delivery category.
    pub delivery_category: Option<DeliveryCategory>,
}

/// Channel information for an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderChannelInfo {
    /// Channel name (e.g., "Online Store", "POS", "Shop").
    pub channel_name: Option<String>,
}

/// Extended order with list view fields.
// Allow: Shopify API Order object has independent boolean properties
// (fully_paid, cancelled, closed, test) that represent separate order states.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderListItem {
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
    /// When order was closed/archived.
    pub closed_at: Option<String>,
    /// When order was cancelled.
    pub cancelled_at: Option<String>,
    /// Financial status.
    pub financial_status: Option<FinancialStatus>,
    /// Fulfillment status.
    pub fulfillment_status: Option<FulfillmentStatus>,
    /// Return status.
    pub return_status: Option<OrderReturnStatus>,
    /// Whether the order is fully paid.
    pub fully_paid: bool,
    /// Whether order is cancelled.
    pub cancelled: bool,
    /// Whether order is closed/archived.
    pub closed: bool,
    /// Whether the order is test mode.
    pub test: bool,
    /// Customer email.
    pub email: Option<String>,
    /// Customer phone.
    pub phone: Option<String>,
    /// Order note.
    pub note: Option<String>,
    /// Order tags.
    pub tags: Vec<String>,
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
    /// Line items (limited for list view).
    pub line_items: Vec<OrderLineItem>,
    /// Total line item quantity.
    pub total_items_quantity: i64,
    /// Fulfillments.
    pub fulfillments: Vec<Fulfillment>,
    /// Billing address.
    pub billing_address: Option<Address>,
    /// Shipping address.
    pub shipping_address: Option<Address>,
    /// Customer ID.
    pub customer_id: Option<String>,
    /// Customer display name.
    pub customer_name: Option<String>,
    /// Order risk assessments.
    pub risks: Vec<OrderRisk>,
    /// Channel information.
    pub channel_info: Option<OrderChannelInfo>,
    /// Shipping line.
    pub shipping_line: Option<OrderShippingLine>,
    /// Applied discount codes.
    pub discount_codes: Vec<String>,
}

/// Paginated list of orders (extended for list view).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderListConnection {
    /// Orders in this page.
    pub orders: Vec<OrderListItem>,
    /// Pagination info.
    pub page_info: PageInfo,
}

// =============================================================================
// Gift Card Types
// =============================================================================

/// A gift card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiftCard {
    /// Gift card ID.
    pub id: String,
    /// Last 4 characters of the code.
    pub last_characters: String,
    /// Current balance.
    pub balance: Money,
    /// Initial value.
    pub initial_value: Money,
    /// Expiration date.
    pub expires_on: Option<String>,
    /// Whether the gift card is enabled.
    pub enabled: bool,
    /// Creation timestamp.
    pub created_at: String,
    /// Associated customer email.
    pub customer_email: Option<String>,
    /// Associated customer name.
    pub customer_name: Option<String>,
    /// Note.
    pub note: Option<String>,
}

/// Paginated list of gift cards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiftCardConnection {
    /// Gift cards in this page.
    pub gift_cards: Vec<GiftCard>,
    /// Pagination info.
    pub page_info: PageInfo,
}

// =============================================================================
// Discount Types
// =============================================================================

/// Discount status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscountStatus {
    /// Discount is active.
    Active,
    /// Discount is expired.
    Expired,
    /// Discount is scheduled.
    Scheduled,
}

/// Type of discount value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DiscountValue {
    /// Percentage discount.
    Percentage { percentage: f64 },
    /// Fixed amount discount.
    FixedAmount { amount: String, currency: String },
}

/// A discount code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountCode {
    /// Discount node ID.
    pub id: String,
    /// Discount title.
    pub title: String,
    /// The actual code customers enter.
    pub code: String,
    /// Discount status.
    pub status: DiscountStatus,
    /// Start date.
    pub starts_at: Option<String>,
    /// End date.
    pub ends_at: Option<String>,
    /// Usage limit.
    pub usage_limit: Option<i64>,
    /// Number of times used.
    pub usage_count: i64,
    /// Discount value.
    pub value: Option<DiscountValue>,
}

/// Paginated list of discount codes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountCodeConnection {
    /// Discount codes in this page.
    pub discount_codes: Vec<DiscountCode>,
    /// Pagination info.
    pub page_info: PageInfo,
}

// =============================================================================
// Fulfillment Order Types
// =============================================================================

/// A fulfillment order represents a group of items that can be fulfilled together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentOrder {
    /// Fulfillment order ID.
    pub id: String,
    /// Fulfillment order status.
    pub status: String,
    /// Location ID where items will be fulfilled from.
    pub location_id: Option<String>,
    /// Location name.
    pub location_name: Option<String>,
    /// Line items in this fulfillment order.
    pub line_items: Vec<FulfillmentOrderLineItem>,
}

/// A line item in a fulfillment order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentOrderLineItem {
    /// Line item ID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// Variant title.
    pub variant_title: Option<String>,
    /// SKU.
    pub sku: Option<String>,
    /// Total quantity.
    pub total_quantity: i64,
    /// Remaining quantity to fulfill.
    pub remaining_quantity: i64,
}

// =============================================================================
// Payout Types
// =============================================================================

/// Payout status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayoutStatus {
    /// Payout is scheduled.
    Scheduled,
    /// Payout is in transit.
    InTransit,
    /// Payout has been paid.
    Paid,
    /// Payout failed.
    Failed,
    /// Payout was canceled.
    Canceled,
}

impl std::fmt::Display for PayoutStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scheduled => write!(f, "Scheduled"),
            Self::InTransit => write!(f, "In Transit"),
            Self::Paid => write!(f, "Paid"),
            Self::Failed => write!(f, "Failed"),
            Self::Canceled => write!(f, "Canceled"),
        }
    }
}

/// A Shopify Payments payout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payout {
    /// Payout ID.
    pub id: String,
    /// Legacy resource ID.
    pub legacy_resource_id: Option<String>,
    /// Payout status.
    pub status: PayoutStatus,
    /// Net amount (the payout amount).
    pub net: Money,
    /// When the payout was issued.
    pub issued_at: Option<String>,
}

/// Connection type for paginated payouts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutConnection {
    /// List of payouts.
    pub payouts: Vec<Payout>,
    /// Pagination info.
    pub page_info: PageInfo,
    /// Current account balance.
    pub balance: Option<Money>,
}

/// Staged upload target for file uploads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedUploadTarget {
    /// The URL to upload the file to.
    pub url: String,
    /// The resource URL after upload completes.
    pub resource_url: String,
    /// Form parameters to include with the upload.
    pub parameters: Vec<(String, String)>,
}

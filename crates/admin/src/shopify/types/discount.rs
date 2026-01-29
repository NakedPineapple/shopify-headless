//! Discount domain types for Shopify Admin API.

use serde::{Deserialize, Serialize};

use super::common::{Money, PageInfo};

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

impl std::fmt::Display for DiscountStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "Active"),
            Self::Expired => write!(f, "Expired"),
            Self::Scheduled => write!(f, "Scheduled"),
        }
    }
}

/// Discount method (how the discount is applied).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscountMethod {
    /// Discount code that customers enter at checkout.
    Code,
    /// Automatic discount applied automatically.
    Automatic,
}

impl std::fmt::Display for DiscountMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Code => write!(f, "Code"),
            Self::Automatic => write!(f, "Automatic"),
        }
    }
}

/// Discount type (what kind of discount).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscountType {
    /// Basic amount off (percentage or fixed).
    Basic,
    /// Buy X Get Y discount.
    BuyXGetY,
    /// Free shipping discount.
    FreeShipping,
}

impl std::fmt::Display for DiscountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Basic => write!(f, "Amount Off"),
            Self::BuyXGetY => write!(f, "Buy X Get Y"),
            Self::FreeShipping => write!(f, "Free Shipping"),
        }
    }
}

/// Sort keys for discount queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscountSortKey {
    /// Sort by title.
    Title,
    /// Sort by creation date.
    #[default]
    CreatedAt,
    /// Sort by last update.
    UpdatedAt,
    /// Sort by start date.
    StartsAt,
    /// Sort by end date.
    EndsAt,
    /// Sort by ID.
    Id,
}

impl DiscountSortKey {
    /// Parse a sort key from a URL parameter string.
    #[must_use]
    pub fn from_str_param(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "title" => Some(Self::Title),
            "created_at" | "created" => Some(Self::CreatedAt),
            "updated_at" | "updated" => Some(Self::UpdatedAt),
            "starts_at" | "start" => Some(Self::StartsAt),
            "ends_at" | "end" => Some(Self::EndsAt),
            "id" => Some(Self::Id),
            _ => None,
        }
    }

    /// Get the URL parameter string for this sort key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::CreatedAt => "created_at",
            Self::UpdatedAt => "updated_at",
            Self::StartsAt => "starts_at",
            Self::EndsAt => "ends_at",
            Self::Id => "id",
        }
    }
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

impl std::fmt::Display for DiscountValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Percentage { percentage } => {
                write!(f, "{}%", (percentage * 100.0).round())
            }
            Self::FixedAmount { amount, currency } => {
                write!(f, "${amount} {currency}")
            }
        }
    }
}

/// Discount combines-with settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscountCombinesWith {
    /// Can combine with order discounts.
    pub order_discounts: bool,
    /// Can combine with product discounts.
    pub product_discounts: bool,
    /// Can combine with shipping discounts.
    pub shipping_discounts: bool,
}

/// Minimum requirement for a discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscountMinimumRequirement {
    /// No minimum requirement.
    None,
    /// Minimum quantity of items.
    Quantity { quantity: String },
    /// Minimum subtotal amount.
    Subtotal { amount: String, currency: String },
}

impl std::fmt::Display for DiscountMinimumRequirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "No minimum"),
            Self::Quantity { quantity } => write!(f, "Min {quantity} items"),
            Self::Subtotal { amount, currency } => write!(f, "Min ${amount} {currency}"),
        }
    }
}

/// Customer eligibility for a discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscountCustomerEligibility {
    /// All customers.
    All,
    /// Specific customer segments.
    Segments { segments: Vec<CustomerSegment> },
    /// Specific customers.
    Customers { customers: Vec<CustomerRef> },
}

/// Reference to a customer segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerSegment {
    /// Segment ID.
    pub id: String,
    /// Segment name.
    pub name: String,
}

/// Reference to a customer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerRef {
    /// Customer ID.
    pub id: String,
    /// Display name.
    pub display_name: String,
    /// Email.
    pub email: Option<String>,
}

/// Items that a discount applies to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscountItems {
    /// All items.
    All,
    /// Specific products.
    Products { products: Vec<ProductRef> },
    /// Specific collections.
    Collections { collections: Vec<CollectionRef> },
}

/// Reference to a product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductRef {
    /// Product ID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// Featured image URL.
    pub image_url: Option<String>,
}

/// Reference to a collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionRef {
    /// Collection ID.
    pub id: String,
    /// Collection title.
    pub title: String,
    /// Collection image URL.
    pub image_url: Option<String>,
}

/// Buy X Get Y requirement type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BxgyRequirementType {
    /// Quantity-based (buy X items).
    Quantity,
    /// Amount-based (spend $X).
    Amount,
}

/// What the customer must buy for a BXGY discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BxgyCustomerBuys {
    /// Requirement type.
    pub requirement_type: BxgyRequirementType,
    /// Quantity required (if quantity-based).
    pub quantity: Option<String>,
    /// Amount required (if amount-based).
    pub amount: Option<String>,
    /// Items that qualify.
    pub items: DiscountItems,
}

/// What the customer gets for a BXGY discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BxgyCustomerGets {
    /// Quantity the customer gets.
    pub quantity: String,
    /// Discount on those items.
    pub discount_value: DiscountValue,
    /// Items they can get.
    pub items: DiscountItems,
}

/// Shipping destination selection for free shipping discounts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShippingDestination {
    /// All countries.
    AllCountries,
    /// Specific countries.
    Countries {
        /// Country codes (ISO 3166-1 alpha-2).
        countries: Vec<String>,
        /// Include rest of world.
        include_rest_of_world: bool,
    },
}

/// A discount code (redeemable code).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountRedeemCode {
    /// Code ID.
    pub id: String,
    /// The code string.
    pub code: String,
    /// Number of times used.
    pub usage_count: i64,
}

/// A discount code (legacy type for backwards compatibility).
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

/// A discount list item (for list view with all types).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountListItem {
    /// Discount node ID.
    pub id: String,
    /// Discount title.
    pub title: String,
    /// Primary code (for code discounts).
    pub code: Option<String>,
    /// Total code count (for code discounts).
    pub code_count: i64,
    /// Discount method (Code or Automatic).
    pub method: DiscountMethod,
    /// Discount type (Basic, `BuyXGetY`, `FreeShipping`).
    pub discount_type: DiscountType,
    /// Discount status.
    pub status: DiscountStatus,
    /// Discount value (for basic discounts).
    pub value: Option<DiscountValue>,
    /// Start date.
    pub starts_at: Option<String>,
    /// End date.
    pub ends_at: Option<String>,
    /// Usage limit (for code discounts).
    pub usage_limit: Option<i64>,
    /// Number of times used.
    pub usage_count: i64,
    /// Once per customer.
    pub once_per_customer: bool,
    /// Combines with settings.
    pub combines_with: DiscountCombinesWith,
    /// Minimum requirement.
    pub minimum_requirement: DiscountMinimumRequirement,
}

/// Full discount details (for detail/edit pages).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountDetail {
    /// Discount node ID.
    pub id: String,
    /// Discount title.
    pub title: String,
    /// Discount method (Code or Automatic).
    pub method: DiscountMethod,
    /// Discount type (Basic, `BuyXGetY`, `FreeShipping`).
    pub discount_type: DiscountType,
    /// Discount status.
    pub status: DiscountStatus,
    /// Start date.
    pub starts_at: Option<String>,
    /// End date.
    pub ends_at: Option<String>,
    /// Usage limit (for code discounts).
    pub usage_limit: Option<i64>,
    /// Number of times used.
    pub usage_count: i64,
    /// Once per customer.
    pub once_per_customer: bool,
    /// Recurring cycle limit (for subscriptions).
    pub recurring_cycle_limit: Option<i64>,
    /// Uses per order limit (for BXGY).
    pub uses_per_order_limit: Option<i64>,
    /// Combines with settings.
    pub combines_with: DiscountCombinesWith,
    /// Minimum requirement.
    pub minimum_requirement: DiscountMinimumRequirement,
    /// Customer eligibility.
    pub customer_eligibility: DiscountCustomerEligibility,
    /// Discount codes (for code discounts).
    pub codes: Vec<DiscountRedeemCode>,
    /// Total code count.
    pub code_count: i64,
    /// Discount value (for basic discounts).
    pub value: Option<DiscountValue>,
    /// Items discount applies to (for basic discounts).
    pub applies_to: Option<DiscountItems>,
    /// Customer buys (for BXGY).
    pub customer_buys: Option<BxgyCustomerBuys>,
    /// Customer gets (for BXGY).
    pub customer_gets: Option<BxgyCustomerGets>,
    /// Shipping destination (for free shipping).
    pub destination: Option<ShippingDestination>,
    /// Maximum shipping price (for free shipping).
    pub max_shipping_price: Option<Money>,
}

/// Paginated list of discount codes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountCodeConnection {
    /// Discount codes in this page.
    pub discount_codes: Vec<DiscountCode>,
    /// Pagination info.
    pub page_info: PageInfo,
}

/// Paginated list of discount list items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountListConnection {
    /// Discounts in this page.
    pub discounts: Vec<DiscountListItem>,
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

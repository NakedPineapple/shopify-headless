//! Domain types for Shopify Storefront API.
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

/// Price range for a product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceRange {
    /// Minimum price among all variants.
    pub min_variant_price: Money,
    /// Maximum price among all variants.
    pub max_variant_price: Money,
}

// =============================================================================
// Image Types
// =============================================================================

/// Product or collection image.
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
// SEO Types
// =============================================================================

/// SEO metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seo {
    /// Page title for search engines.
    pub title: Option<String>,
    /// Meta description.
    pub description: Option<String>,
}

// =============================================================================
// Rating Types
// =============================================================================

/// Product rating data from Judge.me or similar review apps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductRating {
    /// Average rating value (e.g., 4.5).
    pub value: f64,
    /// Minimum rating scale (typically 1.0).
    pub scale_min: f64,
    /// Maximum rating scale (typically 5.0).
    pub scale_max: f64,
    /// Total number of reviews.
    pub count: i64,
}

// =============================================================================
// Selling Plan Types (Subscriptions)
// =============================================================================

/// Price adjustment type for a selling plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SellingPlanPriceAdjustmentValue {
    /// Percentage discount (e.g., 15.0 for 15% off).
    Percentage(f64),
    /// Fixed amount discount.
    FixedAmount(Money),
    /// Fixed price (overrides variant price).
    FixedPrice(Money),
}

/// Price adjustment for a selling plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SellingPlanPriceAdjustment {
    /// The type and value of the adjustment.
    pub adjustment_value: SellingPlanPriceAdjustmentValue,
    /// Number of orders this adjustment applies to (None = all orders).
    pub order_count: Option<i64>,
}

/// An option on a selling plan (e.g., delivery frequency).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SellingPlanOption {
    /// Option name (e.g., "Delivery every").
    pub name: String,
    /// Option value (e.g., "30 days").
    pub value: String,
}

/// A single selling plan (subscription option).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SellingPlan {
    /// Selling plan ID (pass to cart).
    pub id: String,
    /// Display name (e.g., "Delivery every 30 days").
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Options for this plan.
    pub options: Vec<SellingPlanOption>,
    /// Price adjustments (discounts).
    pub price_adjustments: Vec<SellingPlanPriceAdjustment>,
    /// Whether this plan has recurring deliveries.
    pub recurring_deliveries: bool,
}

/// An option for a selling plan group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SellingPlanGroupOption {
    /// Option name (e.g., "Delivery Frequency").
    pub name: String,
    /// Available values (e.g., ["30 days", "60 days", "90 days"]).
    pub values: Vec<String>,
}

/// A group of selling plans (e.g., "Subscribe & Save").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SellingPlanGroup {
    /// Group name (e.g., "Subscribe & Save").
    pub name: String,
    /// Options available in this group.
    pub options: Vec<SellingPlanGroupOption>,
    /// Selling plans in this group.
    pub selling_plans: Vec<SellingPlan>,
}

// =============================================================================
// Product Types
// =============================================================================

/// Selected option on a product variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedOption {
    /// Option name (e.g., "Size", "Color").
    pub name: String,
    /// Selected value (e.g., "Large", "Blue").
    pub value: String,
}

/// Product option definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductOption {
    /// Option ID.
    pub id: String,
    /// Option name (e.g., "Size").
    pub name: String,
    /// Available values (e.g., `["Small", "Medium", "Large"]`).
    pub values: Vec<String>,
}

// =============================================================================
// Shop Pay Types
// =============================================================================

/// Number of installments for Shop Pay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallmentsCount {
    /// Number of payment terms.
    pub count: i64,
}

/// Shop Pay installments pricing for a variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopPayInstallmentsPricing {
    /// Whether the variant is eligible for Shop Pay installments.
    pub eligible: bool,
    /// Price per payment term.
    pub price_per_term: Option<Money>,
    /// Number of installments.
    pub installments_count: Option<InstallmentsCount>,
    /// Full price (total).
    pub full_price: Option<Money>,
}

// =============================================================================
// Product Types (continued)
// =============================================================================

/// A product variant (specific combination of options).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductVariant {
    /// Variant ID.
    pub id: String,
    /// Variant title (combination of option values).
    pub title: String,
    /// Whether this variant is available for sale.
    pub available_for_sale: bool,
    /// Quantity available (if inventory tracking enabled).
    pub quantity_available: Option<i64>,
    /// SKU code.
    pub sku: Option<String>,
    /// Barcode.
    pub barcode: Option<String>,
    /// Current price.
    pub price: Money,
    /// Compare-at price (original price if on sale).
    pub compare_at_price: Option<Money>,
    /// Selected options for this variant.
    pub selected_options: Vec<SelectedOption>,
    /// Variant image.
    pub image: Option<Image>,
    /// Shop Pay installments pricing.
    pub shop_pay_installments: Option<ShopPayInstallmentsPricing>,
}

/// A product in the store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
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
    /// Whether any variant is available.
    pub available_for_sale: bool,
    /// Product type/category.
    #[serde(rename = "product_type")]
    pub kind: String,
    /// Vendor name.
    pub vendor: String,
    /// Product tags.
    pub tags: Vec<String>,
    /// Creation timestamp.
    pub created_at: Option<String>,
    /// Last update timestamp.
    pub updated_at: Option<String>,
    /// Online store URL.
    pub online_store_url: Option<String>,
    /// SEO metadata.
    pub seo: Option<Seo>,
    /// Price range across variants.
    pub price_range: PriceRange,
    /// Compare-at price range.
    pub compare_at_price_range: Option<PriceRange>,
    /// Featured image.
    pub featured_image: Option<Image>,
    /// All product images.
    pub images: Vec<Image>,
    /// Product options.
    pub options: Vec<ProductOption>,
    /// Product variants.
    pub variants: Vec<ProductVariant>,
    /// Product rating from reviews (e.g., Judge.me).
    pub rating: Option<ProductRating>,
    /// Whether product requires a selling plan (subscription-only).
    pub requires_selling_plan: bool,
    /// Selling plan groups (subscription options).
    pub selling_plan_groups: Vec<SellingPlanGroup>,
}

// =============================================================================
// Collection Types
// =============================================================================

/// A collection of products.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    /// Collection ID.
    pub id: String,
    /// URL handle.
    pub handle: String,
    /// Collection title.
    pub title: String,
    /// Plain text description.
    pub description: String,
    /// HTML description.
    pub description_html: String,
    /// Last update timestamp.
    pub updated_at: Option<String>,
    /// Online store URL.
    pub online_store_url: Option<String>,
    /// SEO metadata.
    pub seo: Option<Seo>,
    /// Collection image.
    pub image: Option<Image>,
    /// Products in this collection.
    pub products: Vec<Product>,
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
pub struct ProductConnection {
    /// Products in this page.
    pub products: Vec<Product>,
    /// Pagination info.
    pub page_info: PageInfo,
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
// Cart Types
// =============================================================================

/// Custom attribute (key-value pair).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribute {
    /// Attribute key.
    pub key: String,
    /// Attribute value.
    pub value: Option<String>,
}

/// Input for custom attributes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeInput {
    /// Attribute key.
    pub key: String,
    /// Attribute value.
    pub value: String,
}

/// Merchandise in a cart line (simplified product variant info).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartMerchandise {
    /// Variant ID.
    pub id: String,
    /// Variant title.
    pub title: String,
    /// SKU.
    pub sku: Option<String>,
    /// Whether available for sale.
    pub available_for_sale: bool,
    /// Whether requires shipping.
    pub requires_shipping: bool,
    /// Current price.
    pub price: Money,
    /// Compare-at price.
    pub compare_at_price: Option<Money>,
    /// Selected options.
    pub selected_options: Vec<SelectedOption>,
    /// Variant image.
    pub image: Option<Image>,
    /// Parent product info.
    pub product: CartMerchandiseProduct,
}

/// Simplified product info for cart merchandise.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartMerchandiseProduct {
    /// Product ID.
    pub id: String,
    /// Product handle.
    pub handle: String,
    /// Product title.
    pub title: String,
    /// Vendor.
    pub vendor: String,
    /// Featured image.
    pub featured_image: Option<Image>,
}

/// Cost for a cart line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartLineCost {
    /// Price per unit.
    pub amount_per_quantity: Money,
    /// Compare-at price per unit.
    pub compare_at_amount_per_quantity: Option<Money>,
    /// Subtotal (before discounts).
    pub subtotal_amount: Money,
    /// Total (after discounts).
    pub total_amount: Money,
}

/// A line item in the cart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartLine {
    /// Cart line ID.
    pub id: String,
    /// Quantity.
    pub quantity: i64,
    /// Custom attributes.
    pub attributes: Vec<Attribute>,
    /// Line cost.
    pub cost: CartLineCost,
    /// Product variant.
    pub merchandise: CartMerchandise,
    /// Discount amounts applied to this line.
    pub discount_allocations: Vec<DiscountAllocation>,
}

/// Discount allocation on a cart line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountAllocation {
    /// Amount discounted.
    pub discounted_amount: Money,
}

/// Cart cost summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartCost {
    /// Subtotal before tax/shipping.
    #[serde(rename = "subtotal_amount")]
    pub subtotal: Money,
    /// Total amount.
    #[serde(rename = "total_amount")]
    pub total: Money,
    /// Total tax amount.
    #[serde(rename = "total_tax_amount")]
    pub total_tax: Option<Money>,
    /// Total duty amount.
    #[serde(rename = "total_duty_amount")]
    pub total_duty: Option<Money>,
}

/// Discount code applied to cart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartDiscountCode {
    /// The discount code.
    pub code: String,
    /// Whether the code is applicable.
    pub applicable: bool,
}

/// Customer info in buyer identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartCustomer {
    /// Customer ID.
    pub id: String,
    /// Email.
    pub email: Option<String>,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
}

/// Buyer identity for the cart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartBuyerIdentity {
    /// Email address.
    pub email: Option<String>,
    /// Phone number.
    pub phone: Option<String>,
    /// Country code.
    pub country_code: Option<String>,
    /// Logged-in customer.
    pub customer: Option<CartCustomer>,
}

/// A shopping cart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cart {
    /// Cart ID.
    pub id: String,
    /// Checkout URL.
    pub checkout_url: String,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
    /// Cart note.
    pub note: Option<String>,
    /// Total item quantity.
    pub total_quantity: i64,
    /// Custom attributes.
    pub attributes: Vec<Attribute>,
    /// Buyer identity.
    pub buyer_identity: Option<CartBuyerIdentity>,
    /// Cart cost summary.
    pub cost: CartCost,
    /// Applied discount codes.
    pub discount_codes: Vec<CartDiscountCode>,
    /// Cart lines.
    pub lines: Vec<CartLine>,
}

/// Input for adding a line to cart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartLineInput {
    /// Product variant ID.
    pub merchandise_id: String,
    /// Quantity to add.
    pub quantity: i64,
    /// Custom attributes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<AttributeInput>>,
    /// Selling plan ID (for subscriptions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selling_plan_id: Option<String>,
}

/// Input for updating a cart line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartLineUpdateInput {
    /// Cart line ID.
    pub id: String,
    /// New quantity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<i64>,
    /// New merchandise ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchandise_id: Option<String>,
    /// New attributes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<AttributeInput>>,
    /// New selling plan ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selling_plan_id: Option<String>,
}

/// User error from cart mutations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartUserError {
    /// Error code.
    pub code: Option<String>,
    /// Field path that caused the error.
    pub field: Option<Vec<String>>,
    /// Human-readable error message.
    pub message: String,
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
    /// Sort by last update.
    UpdatedAt,
    /// Sort by creation date.
    CreatedAt,
    /// Sort by best selling.
    BestSelling,
    /// Sort by price.
    Price,
    /// Sort by ID.
    Id,
    /// Sort by relevance (for search).
    Relevance,
}

/// Sort keys for collection product queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProductCollectionSortKey {
    /// Default collection order.
    CollectionDefault,
    /// Sort by title.
    Title,
    /// Sort by price.
    Price,
    /// Sort by best selling.
    BestSelling,
    /// Sort by creation date.
    Created,
    /// Sort by ID.
    Id,
    /// Sort manually.
    Manual,
    /// Sort by relevance.
    Relevance,
}

/// Sort keys for collection queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CollectionSortKey {
    /// Sort by title.
    Title,
    /// Sort by last update.
    UpdatedAt,
    /// Sort by ID.
    Id,
    /// Sort by relevance.
    Relevance,
}

/// Intent for product recommendations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProductRecommendationIntent {
    /// Related products.
    Related,
    /// Complementary products.
    Complementary,
}

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
    /// Available values (e.g., `["30 days", "60 days", "90 days"]`).
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
    /// Product ingredients (from metafield, for beauty products).
    pub ingredients: Option<String>,
    /// Usage directions (from metafield, for beauty products).
    pub directions: Option<String>,
    /// Warning text (from metafield, for beauty products).
    pub warning: Option<String>,
    /// What the product promotes (from metafield, list of strings).
    pub promotes: Vec<String>,
    /// Product benefits (from metafield, rich text).
    pub benefits: Option<String>,
    /// What the product is free from (from metafield, list of strings).
    pub free_from: Vec<String>,
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

// =============================================================================
// Promotion Types (from shop metafield)
// =============================================================================

/// Active promotions configuration from shop metafield.
///
/// This is stored in the `custom.active_promotions` shop metafield as JSON.
/// Structure matches the admin schema in `crates/admin/schemas/active_promotions.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActivePromotions {
    /// Promotion banners to display on cart page (display config only).
    #[serde(default)]
    pub banners: Vec<PromotionBanner>,
    /// Progress tracking display configuration.
    #[serde(default)]
    pub progress_tracking: Vec<ProgressTracking>,
    /// Qualifying rules for discount matching (from Shopify API).
    #[serde(default)]
    pub qualifying_rules: Vec<QualifyingRule>,
}

// =============================================================================
// Banner Types (display only)
// =============================================================================

/// A promotion banner to display on the cart page.
///
/// Contains only display configuration - scheduling comes from the linked qualifying rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionBanner {
    /// Shopify discount node ID (short form, e.g., "1518395293975").
    #[serde(rename = "discount_id")]
    pub id: String,
    /// Display title (e.g., "Free Bronzer with Any Purchase").
    pub title: String,
    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional badge text (e.g., "LIMITED TIME").
    #[serde(default)]
    pub badge_text: Option<String>,
    /// Icon name (e.g., "gift", "truck").
    #[serde(default = "default_icon")]
    pub icon: String,
    /// Accent color (e.g., "honey", "primary").
    #[serde(default = "default_accent_color")]
    pub accent_color: String,
    /// Optional CTA button text.
    #[serde(default)]
    pub cta_text: Option<String>,
    /// Optional CTA button URL.
    #[serde(default)]
    pub cta_url: Option<String>,
    /// Display priority (lower numbers appear first).
    #[serde(default)]
    pub priority: i32,
}

fn default_icon() -> String {
    "gift".to_string()
}

fn default_accent_color() -> String {
    "honey".to_string()
}

// =============================================================================
// Progress Tracking Types (display only)
// =============================================================================

/// Progress tracking display configuration.
///
/// Contains only display configuration - matching logic comes from `QualifyingRule`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressTracking {
    /// Shopify discount node ID (short form, e.g., "1518395293975").
    #[serde(rename = "discount_id")]
    pub id: String,
    /// Icon name (Phosphor icon).
    #[serde(default = "default_icon")]
    pub icon: String,
    /// Accent color.
    #[serde(default = "default_accent_color")]
    pub accent_color: String,
    /// Optional CTA button text.
    #[serde(default)]
    pub cta_text: Option<String>,
    /// Optional CTA button URL.
    #[serde(default)]
    pub cta_url: Option<String>,
    /// Template for suggestion message (use `{needed}` as placeholder).
    #[serde(default = "default_suggestion_template")]
    pub suggestion_template: String,
    /// Optional badge text above suggestion.
    #[serde(default)]
    pub suggestion_badge_text: Option<String>,
    /// Template for qualified message.
    #[serde(default = "default_qualified_template")]
    pub qualified_template: String,
    /// Optional badge text above qualified message.
    #[serde(default)]
    pub qualified_badge_text: Option<String>,
    /// Priority for display ordering (lower = higher priority).
    #[serde(default)]
    pub priority: i32,
    /// Whether to show a progress bar.
    #[serde(default = "default_true")]
    pub show_progress_bar: bool,
    /// Whether to hide when the customer qualifies.
    #[serde(default)]
    pub hide_when_qualified: bool,
}

fn default_suggestion_template() -> String {
    "Add {needed} more to qualify!".to_string()
}

fn default_qualified_template() -> String {
    "You qualify!".to_string()
}

const fn default_true() -> bool {
    true
}

// =============================================================================
// Qualifying Rule Types (matching logic from Shopify API)
// =============================================================================

/// A qualifying rule for discount matching.
///
/// This data is extracted from the Shopify Admin API and should NOT be
/// manually edited. The structure mirrors Shopify's discount types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualifyingRule {
    /// Shopify discount node ID (short form).
    #[serde(rename = "discount_id")]
    pub id: String,
    /// Rule type matching Shopify's discount types.
    #[serde(rename = "type")]
    pub rule_type: QualifyingRuleType,
    /// When the discount starts (from Shopify).
    #[serde(default)]
    pub starts_at: Option<String>,
    /// When the discount ends (from Shopify).
    #[serde(default)]
    pub ends_at: Option<String>,
    /// What discounts this combines with.
    #[serde(default)]
    pub combines_with: Option<CombinesWith>,
    /// Amount off products configuration (for `AMOUNT_OFF_PRODUCTS` type).
    #[serde(default)]
    pub amount_off_products: Option<AmountOffProducts>,
    /// Amount off order configuration (for `AMOUNT_OFF_ORDER` type).
    #[serde(default)]
    pub amount_off_order: Option<AmountOffOrder>,
    /// Buy X Get Y configuration (for `BUY_X_GET_Y` type).
    #[serde(default)]
    pub bxgy: Option<Bxgy>,
    /// Free shipping configuration (for `FREE_SHIPPING` type).
    #[serde(default)]
    pub free_shipping: Option<FreeShipping>,
}

/// Type of qualifying rule (matches Shopify discount types).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QualifyingRuleType {
    /// Percentage or fixed amount off specific products.
    AmountOffProducts,
    /// Buy X Get Y discount.
    BuyXGetY,
    /// Percentage or fixed amount off the entire order.
    AmountOffOrder,
    /// Free shipping threshold.
    FreeShipping,
}

/// What other discounts this discount combines with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinesWith {
    /// Combines with product discounts.
    #[serde(default)]
    pub product_discounts: bool,
    /// Combines with order discounts.
    #[serde(default)]
    pub order_discounts: bool,
    /// Combines with shipping discounts.
    #[serde(default)]
    pub shipping_discounts: bool,
}

/// Configuration for amount off products discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmountOffProducts {
    /// The discount value (percentage or fixed amount).
    pub discount_value: DiscountValue,
    /// Which products the discount applies to.
    #[serde(default)]
    pub applies_to: Option<AppliesTo>,
    /// Minimum requirement to qualify.
    #[serde(default)]
    pub minimum_requirement: Option<MinimumRequirement>,
}

/// Configuration for amount off order discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmountOffOrder {
    /// The discount value (percentage or fixed amount).
    pub discount_value: DiscountValue,
    /// Minimum requirement to qualify.
    #[serde(default)]
    pub minimum_requirement: Option<MinimumRequirement>,
}

/// Configuration for Buy X Get Y discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bxgy {
    /// What the customer must buy to qualify.
    pub customer_buys: CustomerBuys,
    /// What the customer gets.
    pub customer_gets: CustomerGets,
}

/// Configuration for free shipping discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeShipping {
    /// Minimum requirement to qualify.
    #[serde(default)]
    pub minimum_requirement: Option<MinimumRequirement>,
}

/// Discount value (percentage or fixed amount).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountValue {
    /// Type of discount value.
    #[serde(rename = "type")]
    pub value_type: DiscountValueType,
    /// Percentage (0-100) if type is PERCENTAGE.
    #[serde(default)]
    pub percentage: Option<f64>,
    /// Fixed amount if type is `FIXED_AMOUNT`.
    #[serde(default)]
    pub amount: Option<String>,
}

/// Type of discount value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscountValueType {
    /// Percentage off.
    Percentage,
    /// Fixed amount off.
    FixedAmount,
}

/// Which products a discount applies to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliesTo {
    /// Type of targeting.
    #[serde(rename = "type")]
    pub applies_type: AppliesToType,
    /// Collection IDs (for `SPECIFIC_COLLECTIONS`).
    #[serde(default)]
    pub collection_ids: Option<Vec<String>>,
    /// Product IDs (for `SPECIFIC_PRODUCTS`).
    #[serde(default)]
    pub product_ids: Option<Vec<String>>,
}

/// Type of product targeting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppliesToType {
    /// All products.
    All,
    /// Specific collections.
    SpecificCollections,
    /// Specific products.
    SpecificProducts,
}

/// Minimum requirement to qualify for a discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimumRequirement {
    /// Requirement type.
    #[serde(rename = "type")]
    pub requirement_type: MinimumRequirementType,
    /// Required subtotal amount.
    #[serde(default)]
    pub amount: Option<String>,
    /// Required quantity.
    #[serde(default)]
    pub quantity: Option<u32>,
}

/// Type of minimum requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MinimumRequirementType {
    /// No minimum requirement.
    None,
    /// Minimum subtotal amount.
    Amount,
    /// Minimum quantity.
    Quantity,
}

/// Customer buys requirement for BXGY discounts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerBuys {
    /// Requirement type (quantity or amount).
    #[serde(rename = "type")]
    pub requirement_type: BuysRequirementType,
    /// Required quantity (for QUANTITY type).
    #[serde(default)]
    pub quantity: Option<u32>,
    /// Required amount (for AMOUNT type).
    #[serde(default)]
    pub amount: Option<String>,
    /// Which items qualify.
    #[serde(default)]
    pub items: Option<DiscountItems>,
}

/// Requirement type for customer buys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BuysRequirementType {
    /// Quantity-based requirement.
    Quantity,
    /// Amount-based requirement.
    Amount,
}

/// Which items qualify for a discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountItems {
    /// Type of targeting.
    #[serde(rename = "type")]
    pub items_type: DiscountItemsType,
    /// Product IDs (for `SPECIFIC_PRODUCTS`).
    #[serde(default)]
    pub product_ids: Option<Vec<String>>,
    /// Collection IDs (for `SPECIFIC_COLLECTIONS`).
    #[serde(default)]
    pub collection_ids: Option<Vec<String>>,
}

/// Type of discount items targeting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscountItemsType {
    /// Specific products.
    SpecificProducts,
    /// Specific collections.
    SpecificCollections,
}

/// What the customer gets for BXGY discounts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerGets {
    /// Number of items the customer gets.
    #[serde(default = "default_one")]
    pub quantity: u32,
    /// Which items the customer can choose from.
    #[serde(default)]
    pub items: Option<CustomerGetsItems>,
    /// Discount value on the items.
    #[serde(default)]
    pub discount_value: Option<CustomerGetsDiscountValue>,
}

const fn default_one() -> u32 {
    1
}

/// Discount value for customer gets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerGetsDiscountValue {
    /// Type of discount.
    #[serde(rename = "type")]
    pub value_type: CustomerGetsValueType,
    /// Percentage (0-100) if type is PERCENTAGE.
    #[serde(default)]
    pub percentage: Option<f64>,
    /// Fixed amount if type is `AMOUNT_OFF_EACH`.
    #[serde(default)]
    pub amount: Option<String>,
}

/// Type of customer gets discount value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CustomerGetsValueType {
    /// Percentage off.
    Percentage,
    /// Fixed amount off each item.
    AmountOffEach,
    /// Free (100% off).
    Free,
}

/// Which items the customer can receive for a BXGY discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerGetsItems {
    /// Type of targeting.
    #[serde(rename = "type")]
    pub items_type: DiscountItemsType,
    /// Products eligible as gifts (for `SPECIFIC_PRODUCTS`).
    /// Each entry includes `product_id` and `variant_id` for auto-add functionality.
    #[serde(default)]
    pub products: Option<Vec<GiftProduct>>,
    /// Collection IDs (for `SPECIFIC_COLLECTIONS`).
    #[serde(default)]
    pub collection_ids: Option<Vec<String>>,
}

/// A product that can be received as a gift in a BXGY discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiftProduct {
    /// Shopify product GID.
    pub product_id: String,
    /// Shopify variant GID (for cart add operations).
    pub variant_id: String,
}

// =============================================================================
// Cart Recommendations (from shop metafield)
// =============================================================================

/// Cart recommendations configuration from shop metafield.
///
/// This is stored in the `custom.cart_recommendations` shop metafield as JSON.
/// Structure matches the admin schema in `crates/admin/schemas/cart_recommendations.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CartRecommendations {
    /// Product relations defining which products to recommend for each source product.
    #[serde(default)]
    pub product_relations: Vec<ProductRelation>,
}

/// A relation between a source product and its recommended products.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductRelation {
    /// The source product GID (the product in the cart).
    pub product_id: String,
    /// Products to recommend when the source product is in the cart.
    pub related_products: Vec<RelatedProduct>,
}

/// A product recommended when another product is in the cart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedProduct {
    /// The recommended product GID.
    pub product_id: String,
    /// The variant GID (for add-to-cart functionality).
    pub variant_id: String,
}

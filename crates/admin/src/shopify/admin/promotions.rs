//! Promotions management operations for the Admin API.
//!
//! Handles reading/writing the `custom.active_promotions` shop metafield
//! and fetching active automatic discounts.

use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{AdminClient, AdminShopifyError};

// =============================================================================
// Active Promotions Types (matches schemas/active_promotions.json)
// =============================================================================

/// Active promotions configuration stored in shop metafield.
///
/// Structure matches the JSON schema in `schemas/active_promotions.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActivePromotions {
    /// Promotion banners to display on cart page (display config only).
    #[serde(default)]
    pub banners: Vec<PromotionBanner>,
    /// Progress tracking display configuration (display config only).
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
/// Contains only display configuration - no scheduling (that comes from Shopify).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionBanner {
    /// Shopify discount node ID (short form, e.g., "1518395293975").
    #[serde(rename = "discount_id")]
    pub id: String,
    /// Display title.
    pub title: String,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional badge text (e.g., "LIMITED TIME").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge_text: Option<String>,
    /// Icon name (Phosphor icon).
    #[serde(default = "default_icon")]
    pub icon: String,
    /// Accent color.
    #[serde(default = "default_accent_color")]
    pub accent_color: String,
    /// Optional CTA button text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cta_text: Option<String>,
    /// Optional CTA button URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cta_url: Option<String>,
    /// Priority for display ordering (lower = higher priority).
    #[serde(default)]
    pub priority: i32,
}

// =============================================================================
// Progress Tracking Types (display only)
// =============================================================================

/// Progress tracking display configuration.
///
/// Contains only display configuration - matching logic comes from `qualifying_rules`.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cta_text: Option<String>,
    /// Optional CTA button URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cta_url: Option<String>,
    /// Template for suggestion message (e.g., "Add {needed} more for free shipping!").
    #[serde(default = "default_suggestion_template")]
    pub suggestion_template: String,
    /// Optional badge text above suggestion (e.g., "LIMITED TIME").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion_badge_text: Option<String>,
    /// Template for qualified message (e.g., "You qualify for free shipping!").
    #[serde(default = "default_qualified_template")]
    pub qualified_template: String,
    /// Optional badge text above qualified message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

// =============================================================================
// Qualifying Rules Types (matching logic from Shopify API)
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starts_at: Option<String>,
    /// When the discount ends (from Shopify).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ends_at: Option<String>,
    /// What discounts this combines with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combines_with: Option<CombinesWith>,
    /// Amount off products configuration (for `AMOUNT_OFF_PRODUCTS` type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_off_products: Option<AmountOffProducts>,
    /// Amount off order configuration (for `AMOUNT_OFF_ORDER` type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_off_order: Option<AmountOffOrder>,
    /// Buy X Get Y configuration (for `BUY_X_GET_Y` type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bxgy: Option<Bxgy>,
    /// Free shipping configuration (for `FREE_SHIPPING` type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applies_to: Option<AppliesTo>,
    /// Minimum requirement to qualify.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_requirement: Option<MinimumRequirement>,
}

/// Configuration for amount off order discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmountOffOrder {
    /// The discount value (percentage or fixed amount).
    pub discount_value: DiscountValue,
    /// Minimum requirement to qualify.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_requirement: Option<MinimumRequirement>,
}

/// Discount value (percentage or fixed amount).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountValue {
    /// Type of discount value.
    #[serde(rename = "type")]
    pub value_type: DiscountValueType,
    /// Percentage (0-100) if type is `PERCENTAGE`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f64>,
    /// Fixed amount if type is `FIXED_AMOUNT`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_ids: Option<Vec<String>>,
    /// Product IDs (for `SPECIFIC_PRODUCTS`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    /// Required quantity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    /// Required quantity (for `QUANTITY` type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<u32>,
    /// Required amount (for `AMOUNT` type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    /// Which items qualify.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_ids: Option<Vec<String>>,
    /// Collection IDs (for `SPECIFIC_COLLECTIONS`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<CustomerGetsItems>,
    /// Discount value on the items.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discount_value: Option<CustomerGetsDiscountValue>,
}

/// Discount value for customer gets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerGetsDiscountValue {
    /// Type of discount.
    #[serde(rename = "type")]
    pub value_type: CustomerGetsValueType,
    /// Percentage (0-100) if type is `PERCENTAGE`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f64>,
    /// Fixed amount if type is `AMOUNT_OFF_EACH`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub products: Option<Vec<GiftProduct>>,
    /// Collection IDs (for `SPECIFIC_COLLECTIONS`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

const fn default_one() -> u32 {
    1
}

const fn default_true() -> bool {
    true
}

fn default_icon() -> String {
    "gift".to_string()
}

fn default_accent_color() -> String {
    "honey".to_string()
}

// =============================================================================
// Automatic Discount Types (from Shopify API)
// =============================================================================

/// An active automatic discount from Shopify.
#[derive(Debug, Clone)]
pub struct AutomaticDiscount {
    /// Discount node ID (GID).
    pub id: String,
    /// Display title.
    pub title: String,
    /// Discount type.
    pub discount_type: AutomaticDiscountType,
    /// Status (ACTIVE, SCHEDULED, EXPIRED).
    pub status: String,
    /// Start date (ISO 8601).
    pub starts_at: Option<String>,
    /// End date (ISO 8601).
    pub ends_at: Option<String>,
    /// Usage count.
    pub usage_count: i64,
    /// Value description (e.g., "20% off", "Free shipping").
    pub value_description: String,
    /// Minimum requirement description (e.g., "Min $50 subtotal").
    pub minimum_description: Option<String>,
    /// Whether this discount is configured in the `active_promotions` metafield.
    pub is_configured: bool,
}

/// Type of automatic discount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomaticDiscountType {
    /// Basic percentage/amount discount.
    Basic,
    /// Buy X Get Y discount.
    BuyXGetY,
    /// Free shipping discount.
    FreeShipping,
}

impl AutomaticDiscountType {
    /// Get a human-readable label for this discount type.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Basic => "Amount Off",
            Self::BuyXGetY => "Buy X Get Y",
            Self::FreeShipping => "Free Shipping",
        }
    }

    /// Convert to the qualifying rule type.
    #[must_use]
    pub const fn to_rule_type(self) -> QualifyingRuleType {
        match self {
            Self::Basic => QualifyingRuleType::AmountOffProducts,
            Self::BuyXGetY => QualifyingRuleType::BuyXGetY,
            Self::FreeShipping => QualifyingRuleType::FreeShipping,
        }
    }
}

/// Result of fetching active promotions, including the compareDigest for updates.
#[derive(Debug, Clone, Default)]
pub struct ActivePromotionsWithDigest {
    /// The promotions configuration.
    pub promotions: ActivePromotions,
    /// The compareDigest for optimistic concurrency control.
    pub compare_digest: Option<String>,
}

// =============================================================================
// Extracted Rule Data (for building qualifying rules from API)
// =============================================================================

/// Extracted rule data from a Shopify discount.
///
/// Used to build `QualifyingRule` entries from the Shopify API data.
#[derive(Debug, Clone)]
pub struct ExtractedRuleData {
    /// The rule type derived from the discount type.
    pub rule_type: QualifyingRuleType,
    /// When the discount starts.
    pub starts_at: Option<String>,
    /// When the discount ends.
    pub ends_at: Option<String>,
    /// What other discounts this combines with.
    pub combines_with: Option<CombinesWith>,
    /// Amount off products configuration.
    pub amount_off_products: Option<AmountOffProducts>,
    /// Amount off order configuration.
    pub amount_off_order: Option<AmountOffOrder>,
    /// BXGY configuration.
    pub bxgy: Option<Bxgy>,
    /// Free shipping configuration.
    pub free_shipping: Option<FreeShipping>,
}

impl ExtractedRuleData {
    /// Convert to a `QualifyingRule` with the given discount ID.
    #[must_use]
    pub fn into_qualifying_rule(self, discount_id: String) -> QualifyingRule {
        QualifyingRule {
            id: discount_id,
            rule_type: self.rule_type,
            starts_at: self.starts_at,
            ends_at: self.ends_at,
            combines_with: self.combines_with,
            amount_off_products: self.amount_off_products,
            amount_off_order: self.amount_off_order,
            bxgy: self.bxgy,
            free_shipping: self.free_shipping,
        }
    }
}

// =============================================================================
// Admin Client Methods
// =============================================================================

impl AdminClient {
    /// Get the active promotions metafield value with compareDigest.
    ///
    /// Returns default empty promotions if the metafield doesn't exist.
    /// The `compare_digest` should be passed to `set_active_promotions` for updates.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails, JSON parsing fails, or schema validation fails.
    #[instrument(skip(self))]
    pub async fn get_active_promotions_with_digest(
        &self,
    ) -> Result<ActivePromotionsWithDigest, AdminShopifyError> {
        use super::queries::{GetShopMetafield, get_shop_metafield::Variables};
        use super::schema_validation;

        let variables = Variables {
            namespace: "custom".to_string(),
            key: "active_promotions".to_string(),
        };

        let response = self.execute::<GetShopMetafield>(variables).await?;

        let Some(metafield) = response.shop.metafield else {
            return Ok(ActivePromotionsWithDigest::default());
        };

        // Validate the JSON against the schema before parsing
        if let Err(e) = schema_validation::validate_str(&metafield.value) {
            tracing::warn!(error = %e, "Active promotions metafield failed schema validation");
            return Err(AdminShopifyError::ParseError(format!(
                "Schema validation failed: {e}"
            )));
        }

        let promotions: ActivePromotions = serde_json::from_str(&metafield.value)
            .map_err(|e| AdminShopifyError::ParseError(format!("Invalid promotions JSON: {e}")))?;

        Ok(ActivePromotionsWithDigest {
            promotions,
            compare_digest: Some(metafield.compare_digest),
        })
    }

    /// Get the active promotions metafield value.
    ///
    /// Returns `None` if the metafield doesn't exist.
    /// Note: Use `get_active_promotions_with_digest` if you need to update the metafield.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or JSON parsing fails.
    #[instrument(skip(self))]
    pub async fn get_active_promotions(
        &self,
    ) -> Result<Option<ActivePromotions>, AdminShopifyError> {
        let result = self.get_active_promotions_with_digest().await?;
        if result.promotions.banners.is_empty()
            && result.promotions.progress_tracking.is_empty()
            && result.promotions.qualifying_rules.is_empty()
        {
            Ok(None)
        } else {
            Ok(Some(result.promotions))
        }
    }

    /// Get the shop ID for metafield operations.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_shop_id(&self) -> Result<String, AdminShopifyError> {
        let query = serde_json::json!({
            "query": "query GetShopId { shop { id } }"
        });

        let response = self.execute_raw_graphql(query).await?;

        response
            .get("shop")
            .and_then(|s| s.get("id"))
            .and_then(|id| id.as_str())
            .map(String::from)
            .ok_or_else(|| AdminShopifyError::ParseError("Failed to get shop ID".to_string()))
    }

    /// Set the active promotions metafield value.
    ///
    /// Creates the metafield if it doesn't exist.
    /// Pass the `compare_digest` from `get_active_promotions_with_digest` for updates.
    ///
    /// # Errors
    ///
    /// Returns an error if schema validation fails, the API request fails, or returns user errors.
    #[instrument(skip(self, promotions))]
    pub async fn set_active_promotions(
        &self,
        promotions: &ActivePromotions,
        compare_digest: Option<String>,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::{
            SetShopMetafield,
            set_shop_metafield::{MetafieldsSetInput, Variables},
        };
        use super::schema_validation;

        // Get shop ID first
        let shop_id = self.get_shop_id().await?;

        let value = serde_json::to_string(promotions)
            .map_err(|e| AdminShopifyError::ParseError(format!("Failed to serialize: {e}")))?;

        // Log the metafield content for debugging
        tracing::debug!(
            metafield_value = %value,
            "Writing active_promotions metafield to Shopify"
        );

        // Also log pretty-printed for readability
        if let Ok(pretty) = serde_json::to_string_pretty(promotions) {
            tracing::info!(
                banners_count = promotions.banners.len(),
                progress_tracking_count = promotions.progress_tracking.len(),
                qualifying_rules_count = promotions.qualifying_rules.len(),
                "Setting active_promotions metafield:\n{pretty}"
            );
        }

        // Validate the JSON against the schema before writing
        if let Err(e) = schema_validation::validate_str(&value) {
            tracing::error!(error = %e, "Active promotions failed schema validation before write");
            return Err(AdminShopifyError::ParseError(format!(
                "Schema validation failed: {e}"
            )));
        }

        let variables = Variables {
            metafields: vec![MetafieldsSetInput {
                owner_id: shop_id,
                namespace: Some("custom".to_string()),
                key: "active_promotions".to_string(),
                value,
                type_: Some("json".to_string()),
                compare_digest,
            }],
        };

        let response = self.execute::<SetShopMetafield>(variables).await?;

        if let Some(payload) = response.metafields_set
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Get all active automatic discounts from Shopify.
    ///
    /// Only returns automatic discounts (not code-based).
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_automatic_discounts(
        &self,
    ) -> Result<Vec<AutomaticDiscount>, AdminShopifyError> {
        use super::queries::{
            GetActiveAutomaticDiscounts, get_active_automatic_discounts::Variables,
        };

        // Query for automatic discounts only
        let variables = Variables {
            first: Some(50),
            query: Some("method:automatic".to_string()),
        };

        let response = self
            .execute::<GetActiveAutomaticDiscounts>(variables)
            .await?;

        let discounts = response
            .discount_nodes
            .edges
            .into_iter()
            .filter_map(|edge| Self::convert_automatic_discount(edge.node))
            .collect();

        Ok(discounts)
    }

    /// Convert a GraphQL discount node to our domain type.
    fn convert_automatic_discount(
        node: super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNode,
    ) -> Option<AutomaticDiscount> {
        use super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscount as Discount;

        match node.discount {
            Discount::DiscountAutomaticBasic(basic) => {
                let value_desc = Self::format_basic_value(&basic.customer_gets.value);
                let min_desc = basic.minimum_requirement.as_ref().map(Self::format_minimum);

                Some(AutomaticDiscount {
                    id: node.id,
                    title: basic.title,
                    discount_type: AutomaticDiscountType::Basic,
                    status: format!("{:?}", basic.status),
                    starts_at: Some(basic.starts_at),
                    ends_at: basic.ends_at,
                    usage_count: basic.async_usage_count,
                    value_description: value_desc,
                    minimum_description: min_desc,
                    is_configured: false,
                })
            }
            Discount::DiscountAutomaticBxgy(bxgy) => {
                let value_desc = Self::format_bxgy_value(&bxgy);

                Some(AutomaticDiscount {
                    id: node.id,
                    title: bxgy.title,
                    discount_type: AutomaticDiscountType::BuyXGetY,
                    status: format!("{:?}", bxgy.status),
                    starts_at: Some(bxgy.starts_at),
                    ends_at: bxgy.ends_at,
                    usage_count: bxgy.async_usage_count,
                    value_description: value_desc,
                    minimum_description: None,
                    is_configured: false,
                })
            }
            Discount::DiscountAutomaticFreeShipping(fs) => {
                let min_desc = fs.minimum_requirement.as_ref().map(Self::format_fs_minimum);

                Some(AutomaticDiscount {
                    id: node.id,
                    title: fs.title,
                    discount_type: AutomaticDiscountType::FreeShipping,
                    status: format!("{:?}", fs.status),
                    starts_at: Some(fs.starts_at),
                    ends_at: fs.ends_at,
                    usage_count: fs.async_usage_count,
                    value_description: "Free shipping".to_string(),
                    minimum_description: min_desc,
                    is_configured: false,
                })
            }
            _ => None,
        }
    }

    /// Format basic discount value for display.
    fn format_basic_value(
        value: &super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBasicCustomerGetsValue,
    ) -> String {
        use super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBasicCustomerGetsValue as Value;

        match value {
            Value::DiscountPercentage(p) => format!("{}% off", (p.percentage * 100.0).round()),
            Value::DiscountAmount(a) => format!("${} off", a.amount.amount),
            Value::DiscountOnQuantity(q) => {
                // The quantity field is a struct with a quantity String field
                let qty = &q.quantity.quantity;
                format!("{qty} free")
            }
        }
    }

    /// Format BXGY discount value for display.
    fn format_bxgy_value(
        bxgy: &super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBxgy,
    ) -> String {
        use super::queries::get_active_automatic_discounts::{
            GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBxgyCustomerBuysValue as BuysValue,
            GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBxgyCustomerGetsValue as GetsValue,
        };

        let buy_qty = match &bxgy.customer_buys.value {
            BuysValue::DiscountQuantity(q) => q.quantity.clone(),
            BuysValue::DiscountPurchaseAmount(a) => format!("${}", a.amount),
        };

        let get_desc = match &bxgy.customer_gets.value {
            GetsValue::DiscountPercentage(p) => format!("{}% off", (p.percentage * 100.0).round()),
            GetsValue::DiscountAmount(a) => format!("${} off", a.amount.amount),
            GetsValue::DiscountOnQuantity(q) => {
                // The quantity field is a struct with a quantity String field
                let qty = &q.quantity.quantity;
                format!("{qty} free")
            }
        };

        format!("Buy {buy_qty}, get {get_desc}")
    }

    /// Format minimum requirement for display (basic/BXGY).
    fn format_minimum(
        req: &super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBasicMinimumRequirement,
    ) -> String {
        use super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBasicMinimumRequirement as Req;

        match req {
            Req::DiscountMinimumQuantity(q) => {
                format!("Min {} items", q.greater_than_or_equal_to_quantity)
            }
            Req::DiscountMinimumSubtotal(s) => {
                format!("Min ${}", s.greater_than_or_equal_to_subtotal.amount)
            }
        }
    }

    /// Format minimum requirement for display (free shipping).
    fn format_fs_minimum(
        req: &super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticFreeShippingMinimumRequirement,
    ) -> String {
        use super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticFreeShippingMinimumRequirement as Req;

        match req {
            Req::DiscountMinimumQuantity(q) => {
                format!("Min {} items", q.greater_than_or_equal_to_quantity)
            }
            Req::DiscountMinimumSubtotal(s) => {
                format!("Min ${}", s.greater_than_or_equal_to_subtotal.amount)
            }
        }
    }

    /// Get a single automatic discount by short ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or discount not found.
    #[instrument(skip(self))]
    pub async fn get_automatic_discount_by_id(
        &self,
        short_id: &str,
    ) -> Result<Option<AutomaticDiscount>, AdminShopifyError> {
        let discounts = self.get_automatic_discounts().await?;
        Ok(discounts
            .into_iter()
            .find(|d| d.id.ends_with(&format!("/{short_id}")) || d.id == short_id))
    }

    /// Extract rule data from an automatic discount.
    ///
    /// Returns the qualifying rule type and configuration extracted from the Shopify discount.
    ///
    /// # Errors
    ///
    /// Returns an error if the Shopify API request fails.
    #[instrument(skip(self))]
    pub async fn extract_rule_data_from_discount(
        &self,
        short_id: &str,
    ) -> Result<Option<ExtractedRuleData>, AdminShopifyError> {
        use super::queries::{
            GetActiveAutomaticDiscounts, get_active_automatic_discounts::Variables,
        };

        // Query for automatic discounts only
        let variables = Variables {
            first: Some(50),
            query: Some("method:automatic".to_string()),
        };

        let response = self
            .execute::<GetActiveAutomaticDiscounts>(variables)
            .await?;

        // Find the discount by ID
        let node = response
            .discount_nodes
            .edges
            .into_iter()
            .find(|edge| {
                edge.node.id.ends_with(&format!("/{short_id}")) || edge.node.id == short_id
            })
            .map(|edge| edge.node);

        let Some(node) = node else {
            return Ok(None);
        };

        Ok(Some(Self::extract_rule_data(node)))
    }

    /// Extract rule data from a discount node.
    fn extract_rule_data(
        node: super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNode,
    ) -> ExtractedRuleData {
        use super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscount as Discount;

        match node.discount {
            Discount::DiscountAutomaticBasic(basic) => {
                let minimum_requirement = basic
                    .minimum_requirement
                    .as_ref()
                    .map(Self::extract_basic_minimum);

                let discount_value = Self::extract_basic_discount_value(&basic.customer_gets.value);
                let applies_to = Some(Self::extract_basic_applies_to(&basic.customer_gets.items));
                let combines_with = Some(Self::extract_basic_combines_with(&basic.combines_with));

                ExtractedRuleData {
                    rule_type: QualifyingRuleType::AmountOffProducts,
                    starts_at: Some(basic.starts_at),
                    ends_at: basic.ends_at,
                    combines_with,
                    amount_off_products: Some(AmountOffProducts {
                        discount_value,
                        applies_to,
                        minimum_requirement,
                    }),
                    amount_off_order: None,
                    bxgy: None,
                    free_shipping: None,
                }
            }
            Discount::DiscountAutomaticBxgy(bxgy) => {
                let customer_buys = Self::extract_bxgy_customer_buys(&bxgy.customer_buys);
                let customer_gets = Self::extract_bxgy_customer_gets(&bxgy.customer_gets);
                let combines_with = Some(Self::extract_bxgy_combines_with(&bxgy.combines_with));

                ExtractedRuleData {
                    rule_type: QualifyingRuleType::BuyXGetY,
                    starts_at: Some(bxgy.starts_at),
                    ends_at: bxgy.ends_at,
                    combines_with,
                    amount_off_products: None,
                    amount_off_order: None,
                    bxgy: Some(Bxgy {
                        customer_buys,
                        customer_gets,
                    }),
                    free_shipping: None,
                }
            }
            Discount::DiscountAutomaticFreeShipping(fs) => {
                let minimum_requirement = fs
                    .minimum_requirement
                    .as_ref()
                    .map(Self::extract_fs_minimum_req);
                let combines_with = Some(Self::extract_fs_combines_with(&fs.combines_with));

                ExtractedRuleData {
                    rule_type: QualifyingRuleType::FreeShipping,
                    starts_at: Some(fs.starts_at),
                    ends_at: fs.ends_at,
                    combines_with,
                    amount_off_products: None,
                    amount_off_order: None,
                    bxgy: None,
                    free_shipping: Some(FreeShipping {
                        minimum_requirement,
                    }),
                }
            }
            _ => ExtractedRuleData {
                rule_type: QualifyingRuleType::AmountOffProducts,
                starts_at: None,
                ends_at: None,
                combines_with: None,
                amount_off_products: None,
                amount_off_order: None,
                bxgy: None,
                free_shipping: None,
            },
        }
    }

    /// Extract discount value from basic discount.
    fn extract_basic_discount_value(
        value: &super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBasicCustomerGetsValue,
    ) -> DiscountValue {
        use super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBasicCustomerGetsValue as Value;

        match value {
            Value::DiscountPercentage(p) => DiscountValue {
                value_type: DiscountValueType::Percentage,
                percentage: Some(p.percentage * 100.0),
                amount: None,
            },
            Value::DiscountAmount(a) => DiscountValue {
                value_type: DiscountValueType::FixedAmount,
                percentage: None,
                amount: Some(a.amount.amount.clone()),
            },
            Value::DiscountOnQuantity(_) => DiscountValue {
                value_type: DiscountValueType::Percentage,
                percentage: Some(100.0),
                amount: None,
            },
        }
    }

    /// Extract `customer_buys` from BXGY discount.
    fn extract_bxgy_customer_buys(
        buys: &super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBxgyCustomerBuys,
    ) -> CustomerBuys {
        use super::queries::get_active_automatic_discounts::{
            GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBxgyCustomerBuysItems as BuysItems,
            GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBxgyCustomerBuysValue as BuysValue,
        };

        let (requirement_type, quantity, amount) = match &buys.value {
            BuysValue::DiscountQuantity(q) => {
                (BuysRequirementType::Quantity, q.quantity.parse().ok(), None)
            }
            BuysValue::DiscountPurchaseAmount(a) => {
                (BuysRequirementType::Amount, None, Some(a.amount.clone()))
            }
        };

        // Extract items (qualifying products/collections)
        let items = match &buys.items {
            BuysItems::DiscountProducts(products) => {
                let product_ids: Vec<String> = products
                    .products
                    .edges
                    .iter()
                    .map(|e| e.node.id.clone())
                    .collect();
                Some(DiscountItems {
                    items_type: DiscountItemsType::SpecificProducts,
                    product_ids: Some(product_ids),
                    collection_ids: None,
                })
            }
            BuysItems::DiscountCollections(collections) => {
                let collection_ids: Vec<String> = collections
                    .collections
                    .edges
                    .iter()
                    .map(|e| e.node.id.clone())
                    .collect();
                Some(DiscountItems {
                    items_type: DiscountItemsType::SpecificCollections,
                    product_ids: None,
                    collection_ids: Some(collection_ids),
                })
            }
            BuysItems::AllDiscountItems(_) => None, // All items = no restriction
        };

        CustomerBuys {
            requirement_type,
            quantity,
            amount,
            items,
        }
    }

    /// Extract `customer_gets` from BXGY discount.
    fn extract_bxgy_customer_gets(
        gets: &super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBxgyCustomerGets,
    ) -> CustomerGets {
        use super::queries::get_active_automatic_discounts::{
            GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBxgyCustomerGetsItems as GetsItems,
            GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBxgyCustomerGetsValue as GetsValue,
        };

        // Extract discount value
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let discount_value = match &gets.value {
            GetsValue::DiscountPercentage(p) => Some(CustomerGetsDiscountValue {
                value_type: CustomerGetsValueType::Percentage,
                percentage: Some(p.percentage * 100.0),
                amount: None,
            }),
            GetsValue::DiscountAmount(a) => Some(CustomerGetsDiscountValue {
                value_type: CustomerGetsValueType::AmountOffEach,
                percentage: None,
                amount: Some(a.amount.amount.clone()),
            }),
            GetsValue::DiscountOnQuantity(q) => {
                use super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBxgyCustomerGetsValueOnDiscountOnQuantityEffect as Effect;
                match &q.effect {
                    Effect::DiscountPercentage(p) => {
                        let pct = p.percentage * 100.0;
                        if (pct - 100.0).abs() < 0.01 {
                            Some(CustomerGetsDiscountValue {
                                value_type: CustomerGetsValueType::Free,
                                percentage: None,
                                amount: None,
                            })
                        } else {
                            Some(CustomerGetsDiscountValue {
                                value_type: CustomerGetsValueType::Percentage,
                                percentage: Some(pct),
                                amount: None,
                            })
                        }
                    }
                    Effect::DiscountAmount => Some(CustomerGetsDiscountValue {
                        value_type: CustomerGetsValueType::Free,
                        percentage: None,
                        amount: None,
                    }),
                }
            }
        };

        // Extract items (what products the customer receives)
        let items = match &gets.items {
            GetsItems::DiscountProducts(products) => {
                let gift_products: Vec<GiftProduct> = products
                    .products
                    .edges
                    .iter()
                    .filter_map(|e| {
                        // Get the first variant ID if available
                        let variant_id =
                            e.node.variants.edges.first().map(|v| v.node.id.clone())?;
                        Some(GiftProduct {
                            product_id: e.node.id.clone(),
                            variant_id,
                        })
                    })
                    .collect();
                Some(CustomerGetsItems {
                    items_type: DiscountItemsType::SpecificProducts,
                    products: Some(gift_products),
                    collection_ids: None,
                })
            }
            GetsItems::DiscountCollections(collections) => {
                let collection_ids: Vec<String> = collections
                    .collections
                    .edges
                    .iter()
                    .map(|e| e.node.id.clone())
                    .collect();
                Some(CustomerGetsItems {
                    items_type: DiscountItemsType::SpecificCollections,
                    products: None,
                    collection_ids: Some(collection_ids),
                })
            }
            GetsItems::AllDiscountItems(_) => None, // All items = no restriction
        };

        CustomerGets {
            quantity: 1,
            items,
            discount_value,
        }
    }

    /// Extract minimum requirement from basic discount.
    fn extract_basic_minimum(
        req: &super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBasicMinimumRequirement,
    ) -> MinimumRequirement {
        use super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBasicMinimumRequirement as Req;

        match req {
            Req::DiscountMinimumQuantity(q) => MinimumRequirement {
                requirement_type: MinimumRequirementType::Quantity,
                amount: None,
                quantity: q.greater_than_or_equal_to_quantity.parse().ok(),
            },
            Req::DiscountMinimumSubtotal(s) => MinimumRequirement {
                requirement_type: MinimumRequirementType::Amount,
                amount: Some(s.greater_than_or_equal_to_subtotal.amount.clone()),
                quantity: None,
            },
        }
    }

    /// Extract minimum requirement from free shipping discount.
    fn extract_fs_minimum_req(
        req: &super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticFreeShippingMinimumRequirement,
    ) -> MinimumRequirement {
        use super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticFreeShippingMinimumRequirement as Req;

        match req {
            Req::DiscountMinimumQuantity(q) => MinimumRequirement {
                requirement_type: MinimumRequirementType::Quantity,
                amount: None,
                quantity: q.greater_than_or_equal_to_quantity.parse().ok(),
            },
            Req::DiscountMinimumSubtotal(s) => MinimumRequirement {
                requirement_type: MinimumRequirementType::Amount,
                amount: Some(s.greater_than_or_equal_to_subtotal.amount.clone()),
                quantity: None,
            },
        }
    }

    /// Extract `combinesWith` from basic discount.
    const fn extract_basic_combines_with(
        combines: &super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBasicCombinesWith,
    ) -> CombinesWith {
        CombinesWith {
            product_discounts: combines.product_discounts,
            order_discounts: combines.order_discounts,
            shipping_discounts: combines.shipping_discounts,
        }
    }

    /// Extract `combinesWith` from BXGY discount.
    const fn extract_bxgy_combines_with(
        combines: &super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBxgyCombinesWith,
    ) -> CombinesWith {
        CombinesWith {
            product_discounts: combines.product_discounts,
            order_discounts: combines.order_discounts,
            shipping_discounts: combines.shipping_discounts,
        }
    }

    /// Extract `combinesWith` from free shipping discount.
    const fn extract_fs_combines_with(
        combines: &super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticFreeShippingCombinesWith,
    ) -> CombinesWith {
        CombinesWith {
            product_discounts: combines.product_discounts,
            order_discounts: combines.order_discounts,
            shipping_discounts: combines.shipping_discounts,
        }
    }

    /// Extract `appliesTo` from basic discount customer gets items.
    fn extract_basic_applies_to(
        items: &super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBasicCustomerGetsItems,
    ) -> AppliesTo {
        use super::queries::get_active_automatic_discounts::GetActiveAutomaticDiscountsDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBasicCustomerGetsItems as Items;

        match items {
            Items::DiscountProducts(products) => {
                let product_ids: Vec<String> = products
                    .products
                    .edges
                    .iter()
                    .map(|e| e.node.id.clone())
                    .collect();
                AppliesTo {
                    applies_type: AppliesToType::SpecificProducts,
                    product_ids: Some(product_ids),
                    collection_ids: None,
                }
            }
            Items::DiscountCollections(collections) => {
                let collection_ids: Vec<String> = collections
                    .collections
                    .edges
                    .iter()
                    .map(|e| e.node.id.clone())
                    .collect();
                AppliesTo {
                    applies_type: AppliesToType::SpecificCollections,
                    product_ids: None,
                    collection_ids: Some(collection_ids),
                }
            }
            Items::AllDiscountItems(_) => AppliesTo {
                applies_type: AppliesToType::All,
                product_ids: None,
                collection_ids: None,
            },
        }
    }
}

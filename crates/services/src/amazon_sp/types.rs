//! Shared SP-API request/response types.

use serde::{Deserialize, Serialize};

/// Amazon SP-API error response body.
#[derive(Debug, Deserialize)]
pub struct SpApiErrorResponse {
    pub errors: Option<Vec<SpApiError>>,
}

/// Individual error in an SP-API error response.
#[derive(Debug, Deserialize)]
pub struct SpApiError {
    pub code: String,
    pub message: String,
    pub details: Option<String>,
}

/// Marketplace participation (from Sellers API).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceParticipation {
    pub marketplace: Marketplace,
    pub participation: Participation,
}

/// Marketplace metadata.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marketplace {
    pub id: String,
    pub name: String,
    pub country_code: String,
    pub default_language_code: String,
    pub default_currency_code: String,
    pub domain_name: String,
}

/// Seller participation in a marketplace.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Participation {
    pub is_participating: bool,
    pub has_suspended_listings: bool,
}

/// Response from `GET /sellers/v1/marketplaceParticipations`.
#[derive(Debug, Deserialize)]
pub struct GetMarketplaceParticipationsResponse {
    pub payload: Option<Vec<MarketplaceParticipation>>,
    pub errors: Option<Vec<SpApiError>>,
}

/// Amazon credentials for SP-API access (LWA + AWS).
#[derive(Clone)]
pub struct AmazonCredentials {
    /// LWA client ID.
    pub lwa_client_id: String,
    /// LWA client secret.
    pub lwa_client_secret: secrecy::SecretString,
    /// LWA refresh token (long-lived).
    pub lwa_refresh_token: secrecy::SecretString,
    /// AWS IAM access key ID.
    pub aws_access_key_id: String,
    /// AWS IAM secret access key.
    pub aws_secret_access_key: secrecy::SecretString,
    /// Amazon seller ID.
    pub seller_id: String,
    /// Marketplace ID (default: ATVPDKIKX0DER for US).
    pub marketplace_id: String,
}

impl std::fmt::Debug for AmazonCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AmazonCredentials")
            .field("lwa_client_id", &self.lwa_client_id)
            .field("lwa_client_secret", &"[REDACTED]")
            .field("lwa_refresh_token", &"[REDACTED]")
            .field("aws_access_key_id", &self.aws_access_key_id)
            .field("aws_secret_access_key", &"[REDACTED]")
            .field("seller_id", &self.seller_id)
            .field("marketplace_id", &self.marketplace_id)
            .finish()
    }
}

/// LWA token response from Amazon.
#[derive(Debug, Deserialize, Serialize)]
pub struct LwaTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// Cached LWA access token with expiry.
#[derive(Debug, Clone)]
pub struct LwaToken {
    pub access_token: String,
    pub expires_at: i64,
}

impl LwaToken {
    /// Check if the token is expired (with 60-second buffer).
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now >= self.expires_at - 60
    }
}

// ---------------------------------------------------------------------------
// Catalog Items API v2022-04-01
// ---------------------------------------------------------------------------

/// Query parameters for `GET /catalog/2022-04-01/items`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSearchQuery {
    /// Search keywords.
    pub keywords: String,
    /// Comma-separated marketplace IDs.
    pub marketplace_ids: String,
    /// Comma-separated data sets to include.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub included_data: Option<String>,
    /// Results per page (max 20).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u8>,
    /// Pagination token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_token: Option<String>,
}

/// Response from `GET /catalog/2022-04-01/items`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSearchResponse {
    /// Total number of matching results.
    pub number_of_results: Option<i32>,
    /// Pagination tokens.
    pub pagination: Option<CatalogPagination>,
    /// Catalog items.
    pub items: Vec<CatalogItem>,
}

/// Pagination tokens for catalog search.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPagination {
    pub next_token: Option<String>,
    pub previous_token: Option<String>,
}

/// A catalog item from the Catalog Items API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItem {
    /// Amazon Standard Identification Number.
    pub asin: String,
    /// Summary data per marketplace.
    #[serde(default)]
    pub summaries: Vec<CatalogItemSummaryByMarketplace>,
    /// Images per marketplace.
    #[serde(default)]
    pub images: Vec<CatalogItemImagesByMarketplace>,
    /// Identifiers per marketplace (UPC, EAN, etc.).
    #[serde(default)]
    pub identifiers: Vec<CatalogItemIdentifiersByMarketplace>,
    /// Product type per marketplace.
    #[serde(default)]
    pub product_types: Vec<CatalogItemProductType>,
    /// Sales ranks per marketplace.
    #[serde(default)]
    pub sales_ranks: Vec<CatalogItemSalesRanksByMarketplace>,
}

/// Summary data for a catalog item in a specific marketplace.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItemSummaryByMarketplace {
    pub marketplace_id: String,
    pub item_name: Option<String>,
    pub brand: Option<String>,
    pub color: Option<String>,
    pub size_name: Option<String>,
    pub model_number: Option<String>,
    pub manufacturer: Option<String>,
    pub item_classification: Option<String>,
    pub browse_classification: Option<BrowseClassification>,
}

/// Browse classification for a catalog item.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseClassification {
    pub display_name: String,
    pub classification_id: String,
}

/// Images for a catalog item in a specific marketplace.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItemImagesByMarketplace {
    pub marketplace_id: String,
    pub images: Vec<CatalogItemImage>,
}

/// A single catalog item image.
#[derive(Debug, Deserialize)]
pub struct CatalogItemImage {
    pub variant: String,
    pub link: String,
    pub height: Option<i32>,
    pub width: Option<i32>,
}

/// Identifiers for a catalog item in a specific marketplace.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItemIdentifiersByMarketplace {
    pub marketplace_id: String,
    pub identifiers: Vec<CatalogItemIdentifier>,
}

/// A single item identifier (UPC, EAN, etc.).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItemIdentifier {
    pub identifier_type: String,
    pub identifier: String,
}

/// Product type for a catalog item in a specific marketplace.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItemProductType {
    pub marketplace_id: String,
    pub product_type: String,
}

/// Sales ranks for a catalog item in a specific marketplace.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItemSalesRanksByMarketplace {
    pub marketplace_id: String,
    #[serde(default)]
    pub classification_ranks: Vec<CatalogItemClassificationRank>,
    #[serde(default)]
    pub display_group_ranks: Vec<CatalogItemDisplayGroupRank>,
}

/// Classification-level sales rank.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItemClassificationRank {
    pub classification_id: String,
    pub title: String,
    pub rank: i32,
}

/// Display-group-level sales rank.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItemDisplayGroupRank {
    pub website_display_group: String,
    pub title: String,
    pub rank: i32,
}

// ---------------------------------------------------------------------------
// Listings Items API v2021-08-01
// ---------------------------------------------------------------------------

/// Response from `GET /listings/2021-08-01/items/{sellerId}/{sku}`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingItem {
    pub sku: String,
    #[serde(default)]
    pub summaries: Vec<ListingSummary>,
    #[serde(default)]
    pub issues: Vec<ListingIssue>,
    #[serde(default)]
    pub offers: Vec<ListingOffer>,
    #[serde(default)]
    pub fulfillment_availability: Vec<ListingFulfillmentAvailability>,
}

/// Summary data for a listing in a specific marketplace.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingSummary {
    pub marketplace_id: String,
    pub asin: Option<String>,
    pub product_type: Option<String>,
    pub condition_type: Option<String>,
    #[serde(default)]
    pub status: Vec<String>,
    pub item_name: Option<String>,
    pub created_date: Option<String>,
    pub last_updated_date: Option<String>,
    pub main_image: Option<ListingImage>,
}

/// A listing image.
#[derive(Debug, Deserialize)]
pub struct ListingImage {
    pub link: String,
    pub height: Option<i32>,
    pub width: Option<i32>,
}

/// A validation or quality issue on a listing.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingIssue {
    pub code: String,
    pub message: String,
    pub severity: String,
    #[serde(default)]
    pub attribute_names: Vec<String>,
}

/// An offer (pricing) on a listing.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingOffer {
    pub marketplace_id: String,
    pub offer_type: Option<String>,
    pub price: Option<ListingPrice>,
}

/// Price on a listing offer.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingPrice {
    pub currency_code: String,
    pub amount: String,
}

/// Fulfillment availability for a listing.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingFulfillmentAvailability {
    pub fulfillment_channel_code: String,
    pub quantity: Option<i32>,
}

/// Request body for `PUT /listings/2021-08-01/items/{sellerId}/{sku}`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingsItemPutRequest {
    pub product_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirements: Option<String>,
    pub attributes: serde_json::Value,
}

/// Request body for `PATCH /listings/2021-08-01/items/{sellerId}/{sku}`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingsItemPatchRequest {
    pub product_type: String,
    pub patches: Vec<PatchOperation>,
}

/// A single JSON Patch–style operation for listing updates.
#[derive(Debug, Serialize)]
pub struct PatchOperation {
    pub op: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

/// Response from PUT/PATCH/DELETE listings operations.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingsItemSubmissionResponse {
    pub sku: String,
    pub status: String,
    pub submission_id: String,
    #[serde(default)]
    pub issues: Vec<ListingIssue>,
}

// ---------------------------------------------------------------------------
// FBA Inventory API v1
// ---------------------------------------------------------------------------

/// Query parameters for `GET /fba/inventory/v1/summaries`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorySummariesQuery {
    /// Comma-separated marketplace IDs.
    pub marketplace_ids: String,
    /// Granularity type (always "Marketplace").
    pub granularity_type: String,
    /// Granularity ID (marketplace ID).
    pub granularity_id: String,
    /// Pagination token from a previous response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_token: Option<String>,
    /// Filter by seller SKUs (comma-separated, max 50).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seller_skus: Option<String>,
}

/// Response from `GET /fba/inventory/v1/summaries`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetInventorySummariesResponse {
    /// Inventory summaries payload.
    pub payload: Option<InventorySummariesPayload>,
    /// Pagination info.
    pub pagination: Option<InventoryPagination>,
    /// Errors.
    pub errors: Option<Vec<SpApiError>>,
}

/// Inventory summaries payload.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorySummariesPayload {
    /// Granularity of the data.
    pub granularity: Option<InventoryGranularity>,
    /// Inventory summaries.
    pub inventory_summaries: Vec<InventorySummary>,
}

/// Granularity descriptor.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryGranularity {
    pub granularity_type: Option<String>,
    pub granularity_id: Option<String>,
}

/// A single FBA inventory summary for a SKU.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorySummary {
    /// Amazon ASIN.
    pub asin: Option<String>,
    /// FNSKU (Fulfillment Network SKU).
    pub fn_sku: Option<String>,
    /// Seller SKU.
    pub seller_sku: Option<String>,
    /// Product name.
    pub product_name: Option<String>,
    /// Condition (e.g., "`NewItem`").
    pub condition: Option<String>,
    /// Detailed inventory data.
    pub inventory_details: Option<InventoryDetails>,
    /// Last updated timestamp.
    pub last_updated_time: Option<String>,
    /// Total fulfillable quantity.
    pub total_quantity: Option<i32>,
}

/// Detailed FBA inventory breakdown.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryDetails {
    /// Quantity available for fulfillment.
    pub fulfillable_quantity: Option<i32>,
    /// Quantity inbound to FBA (receiving).
    pub inbound_receiving_quantity: Option<i32>,
    /// Quantity inbound to FBA (working).
    pub inbound_working_quantity: Option<i32>,
    /// Quantity inbound to FBA (shipped).
    pub inbound_shipped_quantity: Option<i32>,
    /// Reserved quantity breakdown.
    pub reserved_quantity: Option<ReservedQuantity>,
    /// Unfulfillable quantity breakdown.
    pub unfulfillable_quantity: Option<UnfulfillableQuantity>,
    /// Quantity being researched.
    pub researching_quantity: Option<ResearchingQuantity>,
}

/// Reserved inventory breakdown.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReservedQuantity {
    pub total_reserved_quantity: Option<i32>,
    pub pending_customer_order_quantity: Option<i32>,
    pub pending_transshipment_quantity: Option<i32>,
    pub fc_processing_quantity: Option<i32>,
}

/// Unfulfillable inventory breakdown.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnfulfillableQuantity {
    pub total_unfulfillable_quantity: Option<i32>,
    pub customer_damaged_quantity: Option<i32>,
    pub warehouse_damaged_quantity: Option<i32>,
    pub distributor_damaged_quantity: Option<i32>,
    pub carrier_damaged_quantity: Option<i32>,
    pub defective_quantity: Option<i32>,
    pub expired_quantity: Option<i32>,
}

/// Researching inventory breakdown.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchingQuantity {
    pub total_researching_quantity: Option<i32>,
}

/// Pagination for inventory summaries.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryPagination {
    pub next_token: Option<String>,
}

// ---------------------------------------------------------------------------
// Orders API v0
// ---------------------------------------------------------------------------

/// Query parameters for `GET /orders/v0/orders`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetOrdersQuery {
    /// Marketplace IDs (comma-separated).
    pub marketplace_ids: String,
    /// Only orders created after this date (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_after: Option<String>,
    /// Filter by order statuses (comma-separated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_statuses: Option<String>,
    /// Fulfillment channels (comma-separated: AFN, MFN).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_channels: Option<String>,
    /// Pagination token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_token: Option<String>,
    /// Maximum results per page (1-100).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_results_per_page: Option<i32>,
}

/// Response from `GET /orders/v0/orders`.
#[derive(Debug, Deserialize)]
pub struct GetOrdersResponse {
    pub payload: Option<OrdersList>,
    pub errors: Option<Vec<SpApiError>>,
}

/// Orders list payload.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OrdersList {
    pub orders: Vec<AmazonOrder>,
    pub next_token: Option<String>,
}

/// Response from `GET /orders/v0/orders/{orderId}`.
#[derive(Debug, Deserialize)]
pub struct GetOrderResponse {
    pub payload: Option<AmazonOrder>,
    pub errors: Option<Vec<SpApiError>>,
}

/// An Amazon order.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AmazonOrder {
    pub amazon_order_id: String,
    pub purchase_date: Option<String>,
    pub last_update_date: Option<String>,
    pub order_status: Option<String>,
    pub fulfillment_channel: Option<String>,
    pub sales_channel: Option<String>,
    pub order_channel: Option<String>,
    pub ship_service_level: Option<String>,
    pub order_total: Option<AmazonMoney>,
    pub number_of_items_shipped: Option<i32>,
    pub number_of_items_unshipped: Option<i32>,
    pub payment_method: Option<String>,
    #[serde(default)]
    pub payment_method_details: Vec<String>,
    pub is_replacement_order: Option<bool>,
    pub replaced_order_id: Option<String>,
    pub marketplace_id: Option<String>,
    pub shipment_service_level_category: Option<String>,
    pub order_type: Option<String>,
    pub earliest_ship_date: Option<String>,
    pub latest_ship_date: Option<String>,
    pub earliest_delivery_date: Option<String>,
    pub latest_delivery_date: Option<String>,
    pub is_business_order: Option<bool>,
    pub is_prime: Option<bool>,
    pub is_premium_order: Option<bool>,
    pub is_global_express_enabled: Option<bool>,
    pub is_sold_by_ab: Option<bool>,
    pub is_ispu: Option<bool>,
    pub shipping_address: Option<AmazonAddress>,
    pub buyer_info: Option<BuyerInfo>,
}

/// Monetary amount with currency.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AmazonMoney {
    pub currency_code: Option<String>,
    pub amount: Option<String>,
}

/// Shipping address.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AmazonAddress {
    pub name: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub address_line3: Option<String>,
    pub city: Option<String>,
    pub county: Option<String>,
    pub district: Option<String>,
    pub state_or_region: Option<String>,
    pub postal_code: Option<String>,
    pub country_code: Option<String>,
    pub phone: Option<String>,
}

/// Buyer information.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct BuyerInfo {
    pub buyer_email: Option<String>,
    pub buyer_name: Option<String>,
    pub buyer_county: Option<String>,
    pub buyer_tax_info: Option<serde_json::Value>,
    pub purchase_order_number: Option<String>,
}

/// Response from `GET /orders/v0/orders/{orderId}/orderItems`.
#[derive(Debug, Deserialize)]
pub struct GetOrderItemsResponse {
    pub payload: Option<OrderItemsList>,
    pub errors: Option<Vec<SpApiError>>,
}

/// Order items list payload.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OrderItemsList {
    pub order_items: Vec<AmazonOrderItem>,
    pub next_token: Option<String>,
    pub amazon_order_id: Option<String>,
}

/// A single order item.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AmazonOrderItem {
    #[serde(rename = "ASIN")]
    pub asin: String,
    pub seller_sku: Option<String>,
    pub order_item_id: String,
    pub title: Option<String>,
    pub quantity_ordered: i32,
    pub quantity_shipped: Option<i32>,
    pub item_price: Option<AmazonMoney>,
    pub item_tax: Option<AmazonMoney>,
    pub shipping_price: Option<AmazonMoney>,
    pub shipping_tax: Option<AmazonMoney>,
    pub promotion_discount: Option<AmazonMoney>,
    pub promotion_discount_tax: Option<AmazonMoney>,
    pub is_gift: Option<bool>,
    pub condition_id: Option<String>,
    pub condition_subtype_id: Option<String>,
    pub condition_note: Option<String>,
}

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

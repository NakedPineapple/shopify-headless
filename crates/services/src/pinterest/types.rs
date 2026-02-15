//! Pinterest API v5 request/response types.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Credentials & Auth
// ---------------------------------------------------------------------------

/// Pinterest API credentials for OAuth 2.0 access.
#[derive(Clone)]
pub struct PinterestCredentials {
    /// Pinterest App ID.
    pub app_id: String,
    /// Pinterest App Secret.
    pub app_secret: secrecy::SecretString,
    /// OAuth access token (30-day expiry).
    pub access_token: secrecy::SecretString,
    /// OAuth refresh token (60-day expiry).
    pub refresh_token: secrecy::SecretString,
    /// Ad Account ID (required for Conversions API).
    pub ad_account_id: String,
}

impl std::fmt::Debug for PinterestCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PinterestCredentials")
            .field("app_id", &self.app_id)
            .field("app_secret", &"[REDACTED]")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("ad_account_id", &self.ad_account_id)
            .finish()
    }
}

/// Cached access token with expiry.
#[derive(Debug, Clone)]
pub struct AccessToken {
    pub access_token: String,
    pub expires_at: i64,
}

impl AccessToken {
    /// Check if the token is expired (with 300-second buffer).
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now >= self.expires_at - 300
    }
}

/// Token refresh response from the Pinterest OAuth endpoint.
#[derive(Debug, Deserialize)]
pub struct TokenRefreshResponse {
    pub access_token: String,
    pub token_type: Option<String>,
    /// Seconds until expiry (typically 2,592,000 = 30 days).
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
    pub refresh_token_expires_in: Option<i64>,
    pub scope: Option<String>,
}

// ---------------------------------------------------------------------------
// Error Types
// ---------------------------------------------------------------------------

/// Pinterest API error response.
#[derive(Debug, Deserialize)]
pub struct PinterestApiError {
    pub code: Option<i32>,
    pub message: String,
    pub status: Option<String>,
}

// ---------------------------------------------------------------------------
// User Account
// ---------------------------------------------------------------------------

/// Pinterest user account information.
#[derive(Debug, Clone, Deserialize)]
pub struct UserAccountInfo {
    pub username: Option<String>,
    pub account_type: Option<String>,
    pub profile_image: Option<String>,
    pub website_url: Option<String>,
    pub business_name: Option<String>,
}

// ---------------------------------------------------------------------------
// Catalog Types
// ---------------------------------------------------------------------------

/// A Pinterest catalog.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PinterestCatalog {
    pub id: Option<String>,
    pub name: Option<String>,
    pub catalog_type: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Paginated response for catalogs.
#[derive(Debug, Deserialize)]
pub struct CatalogsPage {
    pub items: Option<Vec<PinterestCatalog>>,
    pub bookmark: Option<String>,
}

/// A Pinterest product feed.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PinterestFeed {
    pub id: Option<String>,
    pub name: Option<String>,
    pub format: Option<String>,
    pub status: Option<String>,
    pub catalog_type: Option<String>,
    pub default_country: Option<String>,
    pub default_locale: Option<String>,
    pub default_currency: Option<String>,
    pub default_availability: Option<String>,
    pub location: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Paginated response for feeds.
#[derive(Debug, Deserialize)]
pub struct FeedsPage {
    pub items: Option<Vec<PinterestFeed>>,
    pub bookmark: Option<String>,
}

/// Feed processing result.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeedProcessingResult {
    pub id: Option<String>,
    pub status: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub ingestion_details: Option<IngestionDetails>,
    pub product_counts: Option<ProductCounts>,
}

/// Ingestion details within a feed processing result.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IngestionDetails {
    pub errors: Option<IngestionErrors>,
    pub info: Option<IngestionInfo>,
}

/// Error details from feed ingestion.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IngestionErrors {
    pub line_level_errors_count: Option<i32>,
    pub file_level_error: Option<String>,
    pub image_download_error: Option<i32>,
    pub validation_error: Option<i32>,
}

/// Info details from feed ingestion.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IngestionInfo {
    pub in_stock: Option<i32>,
    pub out_of_stock: Option<i32>,
    pub preorder: Option<i32>,
}

/// Product counts from feed processing.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProductCounts {
    pub original: Option<i32>,
    pub in_stock: Option<i32>,
}

/// Paginated response for feed processing results.
#[derive(Debug, Deserialize)]
pub struct FeedProcessingResultsPage {
    pub items: Option<Vec<FeedProcessingResult>>,
    pub bookmark: Option<String>,
}

/// A catalog item (product) from Pinterest.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PinterestCatalogItem {
    pub item_id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub link: Option<String>,
    pub image_link: Option<String>,
    pub price: Option<String>,
    pub availability: Option<String>,
    pub google_product_category: Option<String>,
    pub product_type: Option<String>,
    pub brand: Option<String>,
    pub item_group_id: Option<String>,
}

/// Paginated response for catalog items.
#[derive(Debug, Deserialize)]
pub struct CatalogItemsPage {
    pub items: Option<Vec<PinterestCatalogItem>>,
    pub bookmark: Option<String>,
}

// ---------------------------------------------------------------------------
// Conversions API Types
// ---------------------------------------------------------------------------

/// A conversion event to send to the Pinterest Conversions API.
#[derive(Debug, Clone, Serialize)]
pub struct ConversionEvent {
    pub event_name: String,
    pub action_source: String,
    pub event_time: i64,
    pub event_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_source_url: Option<String>,
    pub user_data: ConversionUserData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_data: Option<ConversionCustomData>,
}

/// User data for a conversion event (hashed where required).
#[derive(Debug, Clone, Serialize)]
pub struct ConversionUserData {
    /// SHA256-hashed email address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub em: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_ip_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<Vec<String>>,
}

/// Custom data for a conversion event (commerce fields).
#[derive(Debug, Clone, Serialize)]
pub struct ConversionCustomData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contents: Option<Vec<ConversionContentItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_items: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
}

/// A single item in the conversion event contents array.
#[derive(Debug, Clone, Serialize)]
pub struct ConversionContentItem {
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<i32>,
}

/// Request body for sending conversion events.
#[derive(Debug, Serialize)]
pub struct ConversionEventsRequest {
    pub data: Vec<ConversionEvent>,
}

/// Response from the Conversions API.
#[derive(Debug, Deserialize)]
pub struct ConversionEventsResponse {
    pub num_events_received: Option<i32>,
    pub num_events_processed: Option<i32>,
    pub events: Option<Vec<ConversionEventStatus>>,
}

/// Status of an individual conversion event.
#[derive(Debug, Deserialize)]
pub struct ConversionEventStatus {
    pub status: Option<String>,
    pub error_message: Option<String>,
    pub warning_message: Option<String>,
}

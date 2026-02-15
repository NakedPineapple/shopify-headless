//! Google Merchant Center API request/response types.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Credentials & Auth
// ---------------------------------------------------------------------------

/// Google Merchant Center API credentials for OAuth 2.0.
#[derive(Clone)]
pub struct GoogleMerchantCredentials {
    /// Google Merchant Center ID.
    pub merchant_id: String,
    /// OAuth 2.0 Client ID.
    pub client_id: String,
    /// OAuth 2.0 Client Secret.
    pub client_secret: secrecy::SecretString,
    /// OAuth access token.
    pub access_token: secrecy::SecretString,
    /// OAuth refresh token.
    pub refresh_token: secrecy::SecretString,
}

impl std::fmt::Debug for GoogleMerchantCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleMerchantCredentials")
            .field("merchant_id", &self.merchant_id)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
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

/// Token refresh response from the Google OAuth endpoint.
#[derive(Debug, Deserialize)]
pub struct TokenRefreshResponse {
    pub access_token: String,
    pub token_type: Option<String>,
    /// Seconds until expiry (typically 3600 = 1 hour).
    pub expires_in: Option<i64>,
    pub scope: Option<String>,
}

// ---------------------------------------------------------------------------
// Error Types
// ---------------------------------------------------------------------------

/// Google API error response.
#[derive(Debug, Deserialize)]
pub struct GoogleApiError {
    pub error: Option<GoogleErrorDetail>,
}

/// Error detail within a Google API error.
#[derive(Debug, Deserialize)]
pub struct GoogleErrorDetail {
    pub code: Option<i32>,
    pub message: Option<String>,
    pub status: Option<String>,
}

// ---------------------------------------------------------------------------
// Account Info
// ---------------------------------------------------------------------------

/// Google Merchant Center account information.
#[derive(Debug, Clone, Deserialize)]
pub struct AccountInfo {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "websiteUrl")]
    pub website_url: Option<String>,
    #[serde(rename = "adultContent")]
    pub adult_content: Option<bool>,
}

// ---------------------------------------------------------------------------
// Product Types
// ---------------------------------------------------------------------------

/// A product from Google Merchant Center.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GoogleProduct {
    pub id: Option<String>,
    #[serde(rename = "offerId")]
    pub offer_id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub link: Option<String>,
    #[serde(rename = "imageLink")]
    pub image_link: Option<String>,
    pub price: Option<GooglePrice>,
    pub availability: Option<String>,
    pub brand: Option<String>,
    pub condition: Option<String>,
    #[serde(rename = "googleProductCategory")]
    pub google_product_category: Option<String>,
    #[serde(rename = "productType")]
    pub product_type: Option<String>,
    pub channel: Option<String>,
}

/// Price structure for a Google product.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GooglePrice {
    pub value: Option<String>,
    pub currency: Option<String>,
}

/// Paginated response for product listing.
#[derive(Debug, Deserialize)]
pub struct ProductsPage {
    pub resources: Option<Vec<GoogleProduct>>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

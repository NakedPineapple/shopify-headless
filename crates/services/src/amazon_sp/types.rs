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

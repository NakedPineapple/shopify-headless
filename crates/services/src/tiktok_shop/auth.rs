//! TikTok Shop authentication: token refresh.
//!
//! Access tokens expire after 24 hours. Refresh tokens have no expiry and
//! are used to obtain new access tokens via `GET /api/v2/token/refresh`.

use secrecy::ExposeSecret;

use super::TikTokShopError;
use super::types::{AccessToken, TikTokApiResponse, TikTokShopCredentials, TokenRefreshResponse};

/// TikTok Shop auth endpoint base URL.
const AUTH_BASE: &str = "https://auth.tiktok-shops.com";

/// Refresh the access token using the refresh token.
///
/// Calls `GET /api/v2/token/refresh` with `app_key`, `app_secret`,
/// `refresh_token`, and `grant_type=refresh_token`.
///
/// # Errors
///
/// Returns `TikTokShopError::TokenRefresh` if the request or response
/// parsing fails.
pub async fn refresh_token(
    client: &reqwest::Client,
    credentials: &TikTokShopCredentials,
) -> Result<AccessToken, TikTokShopError> {
    let url = format!("{AUTH_BASE}/api/v2/token/refresh");

    let response = client
        .get(&url)
        .query(&[
            ("app_key", credentials.app_key.as_str()),
            ("app_secret", credentials.app_secret.expose_secret()),
            ("refresh_token", credentials.refresh_token.expose_secret()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| TikTokShopError::TokenRefresh(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());
        return Err(TikTokShopError::TokenRefresh(format!("{status}: {body}")));
    }

    let api_response: TikTokApiResponse<TokenRefreshResponse> = response
        .json()
        .await
        .map_err(|e| TikTokShopError::TokenRefresh(format!("Failed to parse response: {e}")))?;

    if api_response.code != 0 {
        return Err(TikTokShopError::TokenRefresh(format!(
            "code={}: {}",
            api_response.code, api_response.message
        )));
    }

    let data = api_response
        .data
        .ok_or_else(|| TikTokShopError::TokenRefresh("No data in response".into()))?;

    let now = chrono::Utc::now().timestamp();

    Ok(AccessToken {
        access_token: data.access_token,
        expires_at: now + data.access_token_expire_in,
    })
}

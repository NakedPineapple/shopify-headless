//! Pinterest OAuth 2.0 token refresh.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use secrecy::ExposeSecret;

use super::PinterestError;
use super::types::{AccessToken, PinterestCredentials, TokenRefreshResponse};

/// Pinterest OAuth token endpoint.
const TOKEN_URL: &str = "https://api.pinterest.com/v5/oauth/token";

/// Refresh the access token using the refresh token.
///
/// Pinterest uses HTTP Basic auth with `base64(app_id:app_secret)` for the
/// token endpoint, unlike Meta (GET with query params) or TikTok (POST with
/// query params).
///
/// # Errors
///
/// Returns `PinterestError::TokenRefresh` if the request fails.
pub async fn refresh_token(
    client: &reqwest::Client,
    credentials: &PinterestCredentials,
) -> Result<AccessToken, PinterestError> {
    let basic_auth = STANDARD.encode(format!(
        "{}:{}",
        credentials.app_id,
        credentials.app_secret.expose_secret()
    ));

    let response = client
        .post(TOKEN_URL)
        .header("Authorization", format!("Basic {basic_auth}"))
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", credentials.refresh_token.expose_secret()),
        ])
        .send()
        .await
        .map_err(|e| PinterestError::TokenRefresh(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());
        return Err(PinterestError::TokenRefresh(format!("{status}: {body}")));
    }

    let token_response: TokenRefreshResponse = response
        .json()
        .await
        .map_err(|e| PinterestError::TokenRefresh(format!("Failed to parse response: {e}")))?;

    let now = chrono::Utc::now().timestamp();
    // Default to 30 days if expires_in not provided.
    let expires_in = token_response.expires_in.unwrap_or(2_592_000);

    Ok(AccessToken {
        access_token: token_response.access_token,
        expires_at: now + expires_in,
    })
}

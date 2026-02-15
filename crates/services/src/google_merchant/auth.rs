//! Google OAuth 2.0 token refresh.

use secrecy::ExposeSecret;

use super::GoogleMerchantError;
use super::types::{AccessToken, GoogleMerchantCredentials, TokenRefreshResponse};

/// Google OAuth token endpoint.
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// Refresh the access token using the refresh token.
///
/// Google uses form-encoded POST to the token endpoint with
/// `client_id`, `client_secret`, `refresh_token`, and `grant_type`.
///
/// # Errors
///
/// Returns `GoogleMerchantError::TokenRefresh` if the request fails.
pub async fn refresh_token(
    client: &reqwest::Client,
    credentials: &GoogleMerchantCredentials,
) -> Result<AccessToken, GoogleMerchantError> {
    let response = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", &credentials.client_id),
            ("client_secret", credentials.client_secret.expose_secret()),
            ("refresh_token", credentials.refresh_token.expose_secret()),
        ])
        .send()
        .await
        .map_err(|e| GoogleMerchantError::TokenRefresh(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());
        return Err(GoogleMerchantError::TokenRefresh(format!(
            "{status}: {body}"
        )));
    }

    let token_response: TokenRefreshResponse = response
        .json()
        .await
        .map_err(|e| GoogleMerchantError::TokenRefresh(format!("Failed to parse response: {e}")))?;

    let now = chrono::Utc::now().timestamp();
    // Default to 1 hour if expires_in not provided.
    let expires_in = token_response.expires_in.unwrap_or(3600);

    Ok(AccessToken {
        access_token: token_response.access_token,
        expires_at: now + expires_in,
    })
}

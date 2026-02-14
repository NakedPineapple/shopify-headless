//! Meta Commerce authentication: Page Access Token exchange.

use secrecy::ExposeSecret;

use super::MetaCommerceError;
use super::types::{MetaCommerceCredentials, PageAccessToken, TokenExchangeResponse};

/// Graph API base URL.
const GRAPH_API_BASE: &str = "https://graph.facebook.com/v21.0";

/// Exchange a short-lived token for a long-lived page access token.
///
/// Uses `GET /oauth/access_token?grant_type=fb_exchange_token` to get
/// a 60-day token from the current token.
///
/// # Errors
///
/// Returns `MetaCommerceError::TokenExchange` if the request fails.
pub async fn exchange_token(
    client: &reqwest::Client,
    credentials: &MetaCommerceCredentials,
) -> Result<PageAccessToken, MetaCommerceError> {
    let url = format!("{GRAPH_API_BASE}/oauth/access_token");

    let response = client
        .get(&url)
        .query(&[
            ("grant_type", "fb_exchange_token"),
            (
                "fb_exchange_token",
                credentials.page_access_token.expose_secret(),
            ),
            ("client_id", &credentials.app_id),
            ("client_secret", credentials.app_secret.expose_secret()),
        ])
        .send()
        .await
        .map_err(|e| MetaCommerceError::TokenExchange(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());
        return Err(MetaCommerceError::TokenExchange(format!(
            "{status}: {body}"
        )));
    }

    let token_response: TokenExchangeResponse = response
        .json()
        .await
        .map_err(|e| MetaCommerceError::TokenExchange(format!("Failed to parse response: {e}")))?;

    let now = chrono::Utc::now().timestamp();
    // Default to 60 days if expires_in not provided.
    let expires_in = token_response.expires_in.unwrap_or(5_184_000);

    Ok(PageAccessToken {
        access_token: token_response.access_token,
        expires_at: now + expires_in,
    })
}

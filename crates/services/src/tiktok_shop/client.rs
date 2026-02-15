//! TikTok Shop HTTP client.
//!
//! Thread-safe, cheaply cloneable via `Arc`. All requests are signed with
//! HMAC-SHA256 and wrapped in TikTok's `{ code, message, data }` envelope.

use std::sync::Arc;

use secrecy::ExposeSecret;
use tokio::sync::RwLock;
use tracing::instrument;

use super::TikTokShopError;
use super::auth;
use super::signing;
use super::types::{AccessToken, TikTokApiResponse, TikTokShopCredentials};

/// TikTok Shop Open API base URL.
const BASE_URL: &str = "https://open-api.tiktokglobalshop.com";

/// Maximum retry attempts for throttled requests.
const MAX_RETRIES: u32 = 3;

/// TikTok Shop API client.
///
/// Thread-safe, cheaply cloneable via `Arc`. Handles access token refresh
/// and HMAC-SHA256 request signing transparently.
#[derive(Clone)]
pub struct TikTokShopClient {
    inner: Arc<TikTokShopClientInner>,
}

struct TikTokShopClientInner {
    client: reqwest::Client,
    token: RwLock<Option<AccessToken>>,
    credentials: TikTokShopCredentials,
}

impl TikTokShopClient {
    /// Create a new TikTok Shop client with the given credentials.
    #[must_use]
    pub fn new(credentials: TikTokShopCredentials) -> Self {
        Self {
            inner: Arc::new(TikTokShopClientInner {
                client: reqwest::Client::new(),
                token: RwLock::new(None),
                credentials,
            }),
        }
    }

    /// Get the authorized shop ID.
    #[must_use]
    pub fn shop_id(&self) -> &str {
        &self.inner.credentials.shop_id
    }

    /// Get the shop cipher.
    #[must_use]
    pub fn shop_cipher(&self) -> &str {
        &self.inner.credentials.shop_cipher
    }

    /// Test the connection by fetching authorized shop info.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails or credentials are invalid.
    #[instrument(skip(self))]
    pub async fn test_connection(&self) -> Result<super::types::ShopInfoData, TikTokShopError> {
        self.execute_get("/api/shop/get_authorized_shop", &[]).await
    }

    /// Execute a signed GET request to the TikTok Shop API.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self, params))]
    pub async fn execute_get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        params: &[(String, String)],
    ) -> Result<T, TikTokShopError> {
        self.execute_with_retry(reqwest::Method::GET, path, params, Option::<&()>::None)
            .await
    }

    /// Execute a signed POST request to the TikTok Shop API.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self, params, body))]
    pub async fn execute_post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        params: &[(String, String)],
        body: Option<&(impl serde::Serialize + Sync)>,
    ) -> Result<T, TikTokShopError> {
        self.execute_with_retry(reqwest::Method::POST, path, params, body)
            .await
    }

    /// Execute a request with retry logic for 429 (throttling).
    async fn execute_with_retry<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        params: &[(String, String)],
        body: Option<&(impl serde::Serialize + Sync)>,
    ) -> Result<T, TikTokShopError> {
        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            match self.execute_once(&method, path, params, body).await {
                Ok(value) => return Ok(value),
                Err(TikTokShopError::RateLimited(wait)) => {
                    tracing::warn!(
                        attempt = attempt + 1,
                        wait_seconds = wait,
                        path = path,
                        "TikTok Shop API rate limited, retrying"
                    );
                    let delay = std::cmp::min(wait, 30);
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    last_error = Some(TikTokShopError::RateLimited(wait));
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap_or(TikTokShopError::Api {
            code: 429,
            message: "Max retries exceeded".into(),
        }))
    }

    /// Execute a single signed API request.
    async fn execute_once<T: serde::de::DeserializeOwned>(
        &self,
        method: &reqwest::Method,
        path: &str,
        params: &[(String, String)],
        body: Option<&(impl serde::Serialize + Sync)>,
    ) -> Result<T, TikTokShopError> {
        let access_token = self.get_access_token().await?;

        // Build query params with required fields.
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let mut all_params = vec![
            (
                "app_key".to_string(),
                self.inner.credentials.app_key.clone(),
            ),
            ("timestamp".to_string(), timestamp),
            (
                "shop_cipher".to_string(),
                self.inner.credentials.shop_cipher.clone(),
            ),
            ("access_token".to_string(), access_token),
        ];
        all_params.extend_from_slice(params);

        // Serialize body if present.
        let body_str = body.map(|b| serde_json::to_string(b).unwrap_or_default());

        // Sign the request.
        let sign = signing::sign_request(
            &self.inner.credentials.app_secret,
            path,
            &all_params,
            body_str.as_deref(),
        )?;
        all_params.push(("sign".to_string(), sign));

        let url = format!("{BASE_URL}{path}");
        let mut builder = self
            .inner
            .client
            .request(method.clone(), &url)
            .query(&all_params);

        if let Some(b) = body {
            builder = builder.json(b);
        }

        let response = builder.send().await.map_err(TikTokShopError::Http)?;
        self.handle_response(response).await
    }

    /// Get a valid access token, refreshing if necessary.
    async fn get_access_token(&self) -> Result<String, TikTokShopError> {
        {
            let token = self.inner.token.read().await;
            if let Some(ref t) = *token
                && !t.is_expired()
            {
                return Ok(t.access_token.clone());
            }
        }

        // Try to refresh the token.
        if let Ok(new_token) =
            auth::refresh_token(&self.inner.client, &self.inner.credentials).await
        {
            let access_token = new_token.access_token.clone();
            self.inner.token.write().await.replace(new_token);
            Ok(access_token)
        } else {
            // Fall back to stored credential.
            let stored = self.inner.credentials.access_token.expose_secret();
            if stored.is_empty() {
                return Err(TikTokShopError::Unauthorized(
                    "No access token configured".into(),
                ));
            }

            let now = chrono::Utc::now().timestamp();
            let fallback = AccessToken {
                access_token: stored.to_string(),
                // Assume valid for 1 hour as a fallback.
                expires_at: now + 3600,
            };
            let access_token = fallback.access_token.clone();
            self.inner.token.write().await.replace(fallback);
            Ok(access_token)
        }
    }

    /// Handle TikTok API response, unwrapping the envelope.
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, TikTokShopError> {
        let status = response.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(TikTokShopError::RateLimited(60));
        }

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            if let Ok(mut token) = self.inner.token.try_write() {
                *token = None;
            }
            return Err(TikTokShopError::Unauthorized(
                "Invalid or expired credentials".into(),
            ));
        }

        let body = response
            .text()
            .await
            .map_err(|e| TikTokShopError::Parse(format!("Failed to read response: {e}")))?;

        // Parse the TikTok API envelope.
        let envelope: TikTokApiResponse<T> = serde_json::from_str(&body)
            .map_err(|e| TikTokShopError::Parse(format!("Failed to parse response: {e}")))?;

        if envelope.code != 0 {
            return Err(TikTokShopError::Api {
                code: envelope.code,
                message: envelope.message,
            });
        }

        envelope
            .data
            .ok_or_else(|| TikTokShopError::Parse("No data in response".into()))
    }
}

impl std::fmt::Debug for TikTokShopClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TikTokShopClient")
            .field("shop_id", &self.inner.credentials.shop_id)
            .finish_non_exhaustive()
    }
}

//! Google Merchant Center HTTP client.

use std::sync::Arc;

use secrecy::ExposeSecret;
use tokio::sync::RwLock;
use tracing::instrument;

use super::GoogleMerchantError;
use super::auth;
use super::types::{AccessToken, AccountInfo, GoogleApiError, GoogleMerchantCredentials};

/// Google Merchant Center Content API v2.1 base URL.
const BASE_URL: &str = "https://shoppingcontent.googleapis.com/content/v2.1";

/// Maximum retry attempts for throttled requests.
const MAX_RETRIES: u32 = 3;

/// Google Merchant Center API client.
///
/// Thread-safe, cheaply cloneable via `Arc`. Handles OAuth token
/// refresh transparently.
#[derive(Clone)]
pub struct GoogleMerchantClient {
    inner: Arc<GoogleMerchantClientInner>,
}

struct GoogleMerchantClientInner {
    client: reqwest::Client,
    token: RwLock<Option<AccessToken>>,
    credentials: GoogleMerchantCredentials,
}

impl GoogleMerchantClient {
    /// Create a new Google Merchant Center client with the given credentials.
    #[must_use]
    pub fn new(credentials: GoogleMerchantCredentials) -> Self {
        Self {
            inner: Arc::new(GoogleMerchantClientInner {
                client: reqwest::Client::new(),
                token: RwLock::new(None),
                credentials,
            }),
        }
    }

    /// Get the Merchant Center ID.
    #[must_use]
    pub fn merchant_id(&self) -> &str {
        &self.inner.credentials.merchant_id
    }

    /// Test the connection by fetching the account info.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails or credentials are invalid.
    #[instrument(skip(self))]
    pub async fn test_connection(&self) -> Result<AccountInfo, GoogleMerchantError> {
        let merchant_id = &self.inner.credentials.merchant_id;
        let path = format!("/{merchant_id}/accounts/{merchant_id}");
        self.execute_get(&path, None::<&()>).await
    }

    /// Execute a GET request with automatic token handling.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self, query))]
    pub async fn execute_get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&(impl serde::Serialize + Sync)>,
    ) -> Result<T, GoogleMerchantError> {
        self.execute_with_retry(reqwest::Method::GET, path, query, Option::<&()>::None)
            .await
    }

    /// Execute a request with retry logic for 429 (throttling).
    async fn execute_with_retry<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        query: Option<&(impl serde::Serialize + Sync)>,
        body: Option<&(impl serde::Serialize + Sync)>,
    ) -> Result<T, GoogleMerchantError> {
        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            match self.execute_once(&method, path, query, body).await {
                Ok(value) => return Ok(value),
                Err(GoogleMerchantError::RateLimited(wait)) => {
                    tracing::warn!(
                        attempt = attempt + 1,
                        wait_seconds = wait,
                        path = path,
                        "Google API rate limited, retrying"
                    );
                    let delay = std::cmp::min(wait, 30);
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    last_error = Some(GoogleMerchantError::RateLimited(wait));
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap_or(GoogleMerchantError::Api {
            status: 429,
            message: "Max retries exceeded".into(),
        }))
    }

    /// Execute a single API request.
    async fn execute_once<T: serde::de::DeserializeOwned>(
        &self,
        method: &reqwest::Method,
        path: &str,
        query: Option<&(impl serde::Serialize + Sync)>,
        body: Option<&(impl serde::Serialize + Sync)>,
    ) -> Result<T, GoogleMerchantError> {
        let access_token = self.get_access_token().await?;
        let url = format!("{BASE_URL}{path}");

        let mut builder = self
            .inner
            .client
            .request(method.clone(), &url)
            .bearer_auth(&access_token);

        if let Some(q) = query {
            builder = builder.query(q);
        }
        if let Some(b) = body {
            builder = builder.json(b);
        }

        let response = builder.send().await.map_err(GoogleMerchantError::Http)?;
        self.handle_response(response).await
    }

    /// Get a valid access token, refreshing if necessary.
    async fn get_access_token(&self) -> Result<String, GoogleMerchantError> {
        {
            let token = self.inner.token.read().await;
            if let Some(ref t) = *token
                && !t.is_expired()
            {
                return Ok(t.access_token.clone());
            }
        }

        // Try to refresh the token.
        let stored_token = self.inner.credentials.access_token.expose_secret();
        if stored_token.is_empty() {
            return Err(GoogleMerchantError::Unauthorized(
                "No access token configured".into(),
            ));
        }

        if let Ok(new_token) =
            auth::refresh_token(&self.inner.client, &self.inner.credentials).await
        {
            let access_token = new_token.access_token.clone();
            self.inner.token.write().await.replace(new_token);
            Ok(access_token)
        } else {
            // Fall back to using the stored token directly.
            let now = chrono::Utc::now().timestamp();
            let fallback = AccessToken {
                access_token: stored_token.to_string(),
                // Assume valid for 1 hour as a fallback.
                expires_at: now + 3600,
            };
            let access_token = fallback.access_token.clone();
            self.inner.token.write().await.replace(fallback);
            Ok(access_token)
        }
    }

    /// Handle Google API response, parsing errors appropriately.
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, GoogleMerchantError> {
        let status = response.status();

        if status.is_success() {
            return response
                .json()
                .await
                .map_err(|e| GoogleMerchantError::Parse(format!("Failed to parse response: {e}")));
        }

        Err(self.parse_error(response).await)
    }

    /// Parse an error response from the Google API.
    async fn parse_error(&self, response: reqwest::Response) -> GoogleMerchantError {
        let status = response.status().as_u16();

        if status == 429 {
            return GoogleMerchantError::RateLimited(60);
        }

        if status == 401 || status == 403 {
            if let Ok(mut token) = self.inner.token.try_write() {
                *token = None;
            }
            return GoogleMerchantError::Unauthorized("Invalid or expired credentials".into());
        }

        if status == 404 {
            return GoogleMerchantError::NotFound("Resource not found".into());
        }

        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        if let Ok(api_error) = serde_json::from_str::<GoogleApiError>(&body)
            && let Some(detail) = api_error.error
        {
            return GoogleMerchantError::Api {
                status,
                message: detail
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string()),
            };
        }

        GoogleMerchantError::Api {
            status,
            message: body,
        }
    }
}

impl std::fmt::Debug for GoogleMerchantClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleMerchantClient")
            .field("merchant_id", &self.inner.credentials.merchant_id)
            .finish_non_exhaustive()
    }
}

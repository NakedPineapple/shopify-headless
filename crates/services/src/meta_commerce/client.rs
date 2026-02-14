//! Meta Commerce HTTP client.

use std::sync::Arc;

use secrecy::ExposeSecret;
use tokio::sync::RwLock;
use tracing::instrument;

use super::MetaCommerceError;
use super::auth;
use super::types::{
    CommerceAccountInfo, GraphApiErrorResponse, MetaCommerceCredentials, PageAccessToken,
};

/// Graph API base URL (v21.0).
const BASE_URL: &str = "https://graph.facebook.com/v21.0";

/// Maximum retry attempts for throttled requests.
const MAX_RETRIES: u32 = 3;

/// Meta Commerce API client.
///
/// Thread-safe, cheaply cloneable via `Arc`. Handles page access token
/// refresh transparently.
#[derive(Clone)]
pub struct MetaCommerceClient {
    inner: Arc<MetaCommerceClientInner>,
}

struct MetaCommerceClientInner {
    client: reqwest::Client,
    token: RwLock<Option<PageAccessToken>>,
    credentials: MetaCommerceCredentials,
}

impl MetaCommerceClient {
    /// Create a new Meta Commerce client with the given credentials.
    #[must_use]
    pub fn new(credentials: MetaCommerceCredentials) -> Self {
        Self {
            inner: Arc::new(MetaCommerceClientInner {
                client: reqwest::Client::new(),
                token: RwLock::new(None),
                credentials,
            }),
        }
    }

    /// Get the commerce account ID.
    #[must_use]
    pub fn commerce_account_id(&self) -> &str {
        &self.inner.credentials.commerce_account_id
    }

    /// Get the catalog ID.
    #[must_use]
    pub fn catalog_id(&self) -> &str {
        &self.inner.credentials.catalog_id
    }

    /// Get the page ID.
    #[must_use]
    pub fn page_id(&self) -> &str {
        &self.inner.credentials.page_id
    }

    /// Test the connection by fetching the commerce account info.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails or credentials are invalid.
    #[instrument(skip(self))]
    pub async fn test_connection(&self) -> Result<CommerceAccountInfo, MetaCommerceError> {
        let account_id = self.commerce_account_id().to_string();
        let path = format!("/{account_id}");
        self.execute(&path, None::<&()>).await
    }

    /// Execute a GET request to the Graph API with automatic token handling.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self, query))]
    pub async fn execute<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&(impl serde::Serialize + Sync)>,
    ) -> Result<T, MetaCommerceError> {
        self.execute_with_retry(reqwest::Method::GET, path, query, Option::<&()>::None)
            .await
    }

    /// Execute a request with retry logic for 429 (throttling).
    pub(super) async fn execute_with_retry<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        query: Option<&(impl serde::Serialize + Sync)>,
        body: Option<&(impl serde::Serialize + Sync)>,
    ) -> Result<T, MetaCommerceError> {
        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            match self.execute_once(&method, path, query, body).await {
                Ok(value) => return Ok(value),
                Err(MetaCommerceError::RateLimited(wait)) => {
                    tracing::warn!(
                        attempt = attempt + 1,
                        wait_seconds = wait,
                        path = path,
                        "Graph API rate limited, retrying"
                    );
                    let delay = std::cmp::min(wait, 30);
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    last_error = Some(MetaCommerceError::RateLimited(wait));
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap_or(MetaCommerceError::Api {
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
    ) -> Result<T, MetaCommerceError> {
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

        let response = builder.send().await.map_err(MetaCommerceError::Http)?;
        self.handle_response(response).await
    }

    /// Get a valid page access token, refreshing if necessary.
    async fn get_access_token(&self) -> Result<String, MetaCommerceError> {
        {
            let token = self.inner.token.read().await;
            if let Some(ref t) = *token
                && !t.is_expired()
            {
                return Ok(t.access_token.clone());
            }
        }

        // First use: initialize from the stored credential.
        let stored_token = self.inner.credentials.page_access_token.expose_secret();
        if stored_token.is_empty() {
            return Err(MetaCommerceError::Unauthorized(
                "No page access token configured".into(),
            ));
        }

        // Try to exchange for a long-lived token.
        if let Ok(new_token) =
            auth::exchange_token(&self.inner.client, &self.inner.credentials).await
        {
            let access_token = new_token.access_token.clone();
            self.inner.token.write().await.replace(new_token);
            Ok(access_token)
        } else {
            // Fall back to using the stored token directly.
            let now = chrono::Utc::now().timestamp();
            let fallback = PageAccessToken {
                access_token: stored_token.to_string(),
                // Assume valid for 1 hour as a fallback.
                expires_at: now + 3600,
            };
            let access_token = fallback.access_token.clone();
            self.inner.token.write().await.replace(fallback);
            Ok(access_token)
        }
    }

    /// Handle Graph API response, parsing errors appropriately.
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, MetaCommerceError> {
        let status = response.status();

        if status.is_success() {
            return response
                .json()
                .await
                .map_err(|e| MetaCommerceError::Parse(format!("Failed to parse response: {e}")));
        }

        Err(self.parse_error(response).await)
    }

    /// Parse an error response from the Graph API.
    async fn parse_error(&self, response: reqwest::Response) -> MetaCommerceError {
        let status = response.status().as_u16();

        if status == 429 {
            return MetaCommerceError::RateLimited(60);
        }

        if status == 401 || status == 403 {
            if let Ok(mut token) = self.inner.token.try_write() {
                *token = None;
            }
            return MetaCommerceError::Unauthorized("Invalid or expired credentials".into());
        }

        if status == 404 {
            return MetaCommerceError::NotFound("Resource not found".into());
        }

        // Try to parse a structured Graph API error.
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        if let Ok(error_response) = serde_json::from_str::<GraphApiErrorResponse>(&body) {
            // Check for rate-limiting error codes.
            if error_response.error.code == Some(4)
                || error_response.error.code == Some(32)
                || error_response.error.code == Some(613)
            {
                return MetaCommerceError::RateLimited(60);
            }

            return MetaCommerceError::Api {
                status,
                message: error_response.error.message,
            };
        }

        MetaCommerceError::Api {
            status,
            message: body,
        }
    }
}

impl std::fmt::Debug for MetaCommerceClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetaCommerceClient")
            .field(
                "commerce_account_id",
                &self.inner.credentials.commerce_account_id,
            )
            .field("catalog_id", &self.inner.credentials.catalog_id)
            .finish_non_exhaustive()
    }
}

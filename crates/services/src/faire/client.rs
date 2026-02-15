//! Faire HTTP client.

use std::sync::Arc;

use secrecy::ExposeSecret;
use tracing::instrument;

use super::FaireError;
use super::types::{BrandInfo, FaireCredentials};

/// Faire Brand API v2 base URL.
const BASE_URL: &str = "https://www.faire.com/api/v2";

/// Maximum retry attempts for throttled requests.
const MAX_RETRIES: u32 = 3;

/// Faire API client.
///
/// Thread-safe, cheaply cloneable via `Arc`. Uses simple API key auth
/// via the `X-FAIRE-ACCESS-TOKEN` header (no OAuth flow needed).
#[derive(Clone)]
pub struct FaireClient {
    inner: Arc<FaireClientInner>,
}

struct FaireClientInner {
    client: reqwest::Client,
    credentials: FaireCredentials,
}

impl FaireClient {
    /// Create a new Faire client with the given credentials.
    #[must_use]
    pub fn new(credentials: FaireCredentials) -> Self {
        Self {
            inner: Arc::new(FaireClientInner {
                client: reqwest::Client::new(),
                credentials,
            }),
        }
    }

    /// Get the brand ID.
    #[must_use]
    pub fn brand_id(&self) -> &str {
        &self.inner.credentials.brand_id
    }

    /// Test the connection by fetching brand info.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails or credentials are invalid.
    #[instrument(skip(self))]
    pub async fn test_connection(&self) -> Result<BrandInfo, FaireError> {
        self.execute_get("/brand", None::<&()>).await
    }

    /// Execute a GET request with the Faire API token.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self, query))]
    pub async fn execute_get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&(impl serde::Serialize + Sync)>,
    ) -> Result<T, FaireError> {
        self.execute_with_retry(reqwest::Method::GET, path, query, Option::<&()>::None)
            .await
    }

    /// Execute a PUT request with the Faire API token.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self, body))]
    pub async fn execute_put<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &(impl serde::Serialize + Sync),
    ) -> Result<T, FaireError> {
        self.execute_with_retry(reqwest::Method::PUT, path, Option::<&()>::None, Some(body))
            .await
    }

    /// Execute a request with retry logic for 429 (throttling).
    async fn execute_with_retry<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        query: Option<&(impl serde::Serialize + Sync)>,
        body: Option<&(impl serde::Serialize + Sync)>,
    ) -> Result<T, FaireError> {
        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            match self.execute_once(&method, path, query, body).await {
                Ok(value) => return Ok(value),
                Err(FaireError::RateLimited(wait)) => {
                    tracing::warn!(
                        attempt = attempt + 1,
                        wait_seconds = wait,
                        path = path,
                        "Faire API rate limited, retrying"
                    );
                    let delay = std::cmp::min(wait, 30);
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    last_error = Some(FaireError::RateLimited(wait));
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap_or(FaireError::Api {
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
    ) -> Result<T, FaireError> {
        let url = format!("{BASE_URL}{path}");
        let api_token = self.inner.credentials.api_token.expose_secret();

        let mut builder = self
            .inner
            .client
            .request(method.clone(), &url)
            .header("X-FAIRE-ACCESS-TOKEN", api_token);

        if let Some(q) = query {
            builder = builder.query(q);
        }
        if let Some(b) = body {
            builder = builder.json(b);
        }

        let response = builder.send().await.map_err(FaireError::Http)?;
        self.handle_response(response).await
    }

    /// Handle Faire API response, parsing errors appropriately.
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, FaireError> {
        let status = response.status();

        if status.is_success() {
            return response
                .json()
                .await
                .map_err(|e| FaireError::Parse(format!("Failed to parse response: {e}")));
        }

        Err(self.parse_error(response).await)
    }

    /// Parse an error response from the Faire API.
    async fn parse_error(&self, response: reqwest::Response) -> FaireError {
        let status = response.status().as_u16();

        if status == 429 {
            return FaireError::RateLimited(10);
        }

        if status == 401 || status == 403 {
            return FaireError::Unauthorized("Invalid or expired API token".into());
        }

        if status == 404 {
            return FaireError::NotFound("Resource not found".into());
        }

        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        FaireError::Api {
            status,
            message: body,
        }
    }
}

impl std::fmt::Debug for FaireClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FaireClient")
            .field("brand_id", &self.inner.credentials.brand_id)
            .finish_non_exhaustive()
    }
}

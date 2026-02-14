//! Amazon SP-API HTTP client.

use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::instrument;

use super::AmazonSpError;
use super::auth;
use super::types::{
    AmazonCredentials, GetMarketplaceParticipationsResponse, LwaToken, MarketplaceParticipation,
};

/// SP-API base URL (North America).
const BASE_URL: &str = "https://sellingpartnerapi-na.amazon.com";

/// Maximum retry attempts for throttled requests.
const MAX_RETRIES: u32 = 3;

/// Amazon SP-API client.
///
/// Thread-safe, cheaply cloneable via `Arc`. Handles LWA token refresh
/// and AWS `SigV4` request signing transparently.
#[derive(Clone)]
pub struct AmazonSpClient {
    inner: Arc<AmazonSpClientInner>,
}

struct AmazonSpClientInner {
    client: reqwest::Client,
    token: RwLock<Option<LwaToken>>,
    credentials: AmazonCredentials,
}

impl AmazonSpClient {
    /// Create a new SP-API client with the given credentials.
    #[must_use]
    pub fn new(credentials: AmazonCredentials) -> Self {
        Self {
            inner: Arc::new(AmazonSpClientInner {
                client: reqwest::Client::new(),
                token: RwLock::new(None),
                credentials,
            }),
        }
    }

    /// Get the seller ID.
    #[must_use]
    pub fn seller_id(&self) -> &str {
        &self.inner.credentials.seller_id
    }

    /// Get the marketplace ID.
    #[must_use]
    pub fn marketplace_id(&self) -> &str {
        &self.inner.credentials.marketplace_id
    }

    /// Test the connection by calling the Sellers API.
    ///
    /// Returns the list of marketplace participations for the seller.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails or credentials are invalid.
    #[instrument(skip(self))]
    pub async fn test_connection(&self) -> Result<Vec<MarketplaceParticipation>, AmazonSpError> {
        let response: GetMarketplaceParticipationsResponse = self
            .execute("/sellers/v1/marketplaceParticipations", None::<&()>)
            .await?;

        if let Some(errors) = response.errors
            && let Some(first) = errors.first()
        {
            return Err(AmazonSpError::Api {
                status: 400,
                message: first.message.clone(),
            });
        }

        response.payload.ok_or_else(|| {
            AmazonSpError::Parse("Missing payload in marketplace participations response".into())
        })
    }

    /// Execute a GET request to the SP-API with automatic token refresh and signing.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self, query))]
    pub async fn execute<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&(impl serde::Serialize + Sync)>,
    ) -> Result<T, AmazonSpError> {
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
    ) -> Result<T, AmazonSpError> {
        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            match self.execute_once(&method, path, query, body).await {
                Ok(value) => return Ok(value),
                Err(AmazonSpError::RateLimited(wait)) => {
                    tracing::warn!(
                        attempt = attempt + 1,
                        wait_seconds = wait,
                        path = path,
                        "SP-API rate limited, retrying"
                    );
                    let delay = std::cmp::min(wait, 30);
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    last_error = Some(AmazonSpError::RateLimited(wait));
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap_or(AmazonSpError::Api {
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
    ) -> Result<T, AmazonSpError> {
        let access_token = self.get_access_token().await?;
        let url = format!("{BASE_URL}{path}");

        let mut builder = self
            .inner
            .client
            .request(method.clone(), &url)
            .header("x-amz-access-token", &access_token)
            .header("host", "sellingpartnerapi-na.amazon.com");

        if let Some(q) = query {
            builder = builder.query(q);
        }
        if let Some(b) = body {
            builder = builder.json(b);
        }

        let mut request = builder.build().map_err(AmazonSpError::Http)?;

        auth::sign_request(&mut request, &self.inner.credentials)?;

        let response = self.inner.client.execute(request).await?;
        self.handle_response(response).await
    }

    /// Get a valid LWA access token, refreshing if necessary.
    async fn get_access_token(&self) -> Result<String, AmazonSpError> {
        {
            let token = self.inner.token.read().await;
            if let Some(ref t) = *token
                && !t.is_expired()
            {
                return Ok(t.access_token.clone());
            }
        }

        let new_token =
            auth::exchange_refresh_token(&self.inner.client, &self.inner.credentials).await?;

        let access_token = new_token.access_token.clone();
        {
            let mut token = self.inner.token.write().await;
            *token = Some(new_token);
        }

        Ok(access_token)
    }

    /// Handle SP-API response, parsing errors appropriately.
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, AmazonSpError> {
        let status = response.status();

        if status.is_success() {
            return response
                .json()
                .await
                .map_err(|e| AmazonSpError::Parse(format!("Failed to parse response: {e}")));
        }

        Err(self.parse_error(response).await)
    }

    /// Parse an error response from SP-API.
    async fn parse_error(&self, response: reqwest::Response) -> AmazonSpError {
        let status = response.status().as_u16();

        if status == 429 {
            return AmazonSpError::RateLimited(parse_rate_limit_wait(&response));
        }

        if status == 401 || status == 403 {
            if let Ok(mut token) = self.inner.token.try_write() {
                *token = None;
            }
            return AmazonSpError::Unauthorized("Invalid or expired credentials".into());
        }

        if status == 404 {
            return AmazonSpError::NotFound("Resource not found".into());
        }

        let message = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        AmazonSpError::Api { status, message }
    }
}

/// Parse rate limit wait time from SP-API response headers.
fn parse_rate_limit_wait(response: &reqwest::Response) -> u64 {
    response
        .headers()
        .get("x-amzn-ratelimit-limit")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<f64>().ok())
        .and_then(rate_to_wait_seconds)
        .unwrap_or(2)
}

/// Convert a rate (requests/second) to a wait time in seconds.
///
/// Returns `None` for non-positive or non-finite rates.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "wait is verified finite and non-negative; values above u64::MAX are unreachable from 1/rate"
)]
fn rate_to_wait_seconds(rate: f64) -> Option<u64> {
    if rate <= 0.0 {
        return None;
    }
    // rate is requests/second; wait = 1/rate seconds
    let wait = (1.0 / rate).ceil();
    if wait.is_finite() && wait >= 0.0 {
        Some((wait as u64).max(1))
    } else {
        None
    }
}

impl std::fmt::Debug for AmazonSpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AmazonSpClient")
            .field("seller_id", &self.inner.credentials.seller_id)
            .field("marketplace_id", &self.inner.credentials.marketplace_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url() {
        assert_eq!(BASE_URL, "https://sellingpartnerapi-na.amazon.com");
    }
}

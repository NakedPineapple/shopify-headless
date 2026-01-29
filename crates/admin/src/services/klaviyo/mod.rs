//! Klaviyo API client for newsletter and SMS campaign management.
//!
//! Provides access to Klaviyo's API for managing email and SMS campaigns.
//! Klaviyo handles subscriber sync via native Shopify integration.
//!
//! # Supported Features
//!
//! - **Email campaigns**: HTML newsletters with subject lines
//! - **SMS campaigns**: Text messages (160 char limit, 70 with emoji)
//! - **Subscriber lists**: Email and SMS subscriber management
//!
//! # API Reference
//!
//! - Base URL: `https://a.klaviyo.com/api`
//! - Authentication: Private API key via `Authorization: Klaviyo-API-Key <key>`
//! - API Version: `2024-10-15` (specified via `revision` header)

mod campaigns;
mod types;

pub use campaigns::*;
pub use types::*;

use std::sync::Arc;

use reqwest::header::{HeaderMap, HeaderValue};
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

use crate::config::KlaviyoConfig;

/// Klaviyo API version (revision header).
const API_REVISION: &str = "2024-10-15";

/// Klaviyo API base URL.
const BASE_URL: &str = "https://a.klaviyo.com/api";

/// Errors that can occur when interacting with Klaviyo API.
#[derive(Debug, Error)]
pub enum KlaviyoError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// API returned an error response.
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    /// Rate limited by Klaviyo.
    #[error("Rate limited, retry after {0} seconds")]
    RateLimited(u64),

    /// Resource not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Failed to parse response.
    #[error("Parse error: {0}")]
    Parse(String),

    /// Unauthorized (invalid API key).
    #[error("Unauthorized: invalid API key")]
    Unauthorized,
}

/// Klaviyo API client.
///
/// Provides methods for managing campaigns and accessing subscriber lists.
/// Subscribers are synced automatically via Klaviyo's Shopify integration.
#[derive(Clone)]
pub struct KlaviyoClient {
    inner: Arc<KlaviyoClientInner>,
}

struct KlaviyoClientInner {
    client: reqwest::Client,
    list_id: String,
}

impl KlaviyoClient {
    /// Create a new Klaviyo API client.
    ///
    /// # Errors
    ///
    /// Returns error if the HTTP client fails to build.
    pub fn new(config: &KlaviyoConfig) -> Result<Self, KlaviyoError> {
        let mut headers = HeaderMap::new();

        // Authorization header
        let auth_value = format!("Klaviyo-API-Key {}", config.api_key.expose_secret());
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&auth_value)
                .map_err(|e| KlaviyoError::Parse(format!("Invalid API key format: {e}")))?,
        );

        // Revision header for API versioning
        headers.insert("revision", HeaderValue::from_static(API_REVISION));

        // Content-Type for JSON:API
        headers.insert(
            "Content-Type",
            HeaderValue::from_static("application/vnd.api+json"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            inner: Arc::new(KlaviyoClientInner {
                client,
                list_id: config.list_id.clone(),
            }),
        })
    }

    /// Get the configured newsletter list ID.
    #[must_use]
    pub fn list_id(&self) -> &str {
        &self.inner.list_id
    }

    /// Execute a GET request to the Klaviyo API.
    pub(crate) async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<T, KlaviyoError> {
        let url = format!("{BASE_URL}{path}");
        let response = self.inner.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Execute a POST request to the Klaviyo API.
    pub(crate) async fn post<T: serde::de::DeserializeOwned, B: serde::Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, KlaviyoError> {
        let url = format!("{BASE_URL}{path}");
        let response = self.inner.client.post(&url).json(body).send().await?;
        self.handle_response(response).await
    }

    /// Execute a PATCH request to the Klaviyo API.
    pub(crate) async fn patch<T: serde::de::DeserializeOwned, B: serde::Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, KlaviyoError> {
        let url = format!("{BASE_URL}{path}");
        let response = self.inner.client.patch(&url).json(body).send().await?;
        self.handle_response(response).await
    }

    /// Execute a DELETE request to the Klaviyo API.
    pub(crate) async fn delete(&self, path: &str) -> Result<(), KlaviyoError> {
        let url = format!("{BASE_URL}{path}");
        let response = self.inner.client.delete(&url).send().await?;

        let status = response.status();
        if status.is_success() || status.as_u16() == 204 {
            return Ok(());
        }

        Err(self.parse_error(response).await)
    }

    /// Handle API response and parse JSON.
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, KlaviyoError> {
        let status = response.status();

        if status.is_success() {
            return response
                .json()
                .await
                .map_err(|e| KlaviyoError::Parse(format!("Failed to parse response: {e}")));
        }

        Err(self.parse_error(response).await)
    }

    /// Parse error response from Klaviyo API.
    async fn parse_error(&self, response: reqwest::Response) -> KlaviyoError {
        let status = response.status().as_u16();

        // Check for rate limiting
        if status == 429 {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(60);
            return KlaviyoError::RateLimited(retry_after);
        }

        // Check for unauthorized
        if status == 401 || status == 403 {
            return KlaviyoError::Unauthorized;
        }

        // Check for not found
        if status == 404 {
            return KlaviyoError::NotFound("Resource not found".to_string());
        }

        // Try to parse error message from response body
        let message = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        KlaviyoError::Api { status, message }
    }
}

impl std::fmt::Debug for KlaviyoClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KlaviyoClient")
            .field("list_id", &self.inner.list_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_constants() {
        assert_eq!(BASE_URL, "https://a.klaviyo.com/api");
        assert!(!API_REVISION.is_empty());
    }
}

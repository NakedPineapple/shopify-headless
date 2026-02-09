//! Lightweight Shopify Admin API client for email triage context.
//!
//! Reads the OAuth token from the `admin.shopify_token` table (shared with
//! the admin binary) and makes raw GraphQL queries for order and product lookups.

use std::sync::Arc;

use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;
use tokio::sync::RwLock;
use tracing::{debug, instrument, warn};

use crate::config::ShopifyConfig;

/// Error type for Shopify API operations.
#[derive(Debug, thiserror::Error)]
pub enum ShopifyError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// GraphQL returned errors.
    #[error("GraphQL error: {0}")]
    GraphQL(String),

    /// No OAuth token available.
    #[error("no Shopify OAuth token configured — complete OAuth flow in admin")]
    NoToken,

    /// Database error loading token.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Lightweight Shopify Admin API client using raw GraphQL queries.
///
/// Loads the OAuth access token from the `admin.shopify_token` table and
/// caches it in memory. Does not perform the OAuth flow itself — that is
/// handled by the admin binary.
#[derive(Clone)]
pub struct ShopifyClient {
    inner: Arc<ShopifyClientInner>,
}

struct ShopifyClientInner {
    client: reqwest::Client,
    store: String,
    api_version: String,
    token: RwLock<Option<SecretString>>,
}

impl ShopifyClient {
    /// Create a new Shopify client.
    #[must_use]
    pub fn new(config: &ShopifyConfig) -> Self {
        Self {
            inner: Arc::new(ShopifyClientInner {
                client: reqwest::Client::new(),
                store: config.store.clone(),
                api_version: config.api_version.clone(),
                token: RwLock::new(None),
            }),
        }
    }

    /// Load the OAuth token from the database. Call once at startup.
    #[instrument(skip(self, pool))]
    pub async fn load_token(&self, pool: &PgPool) -> Result<(), ShopifyError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT access_token FROM admin.shopify_token WHERE shop = $1")
                .bind(&self.inner.store)
                .fetch_optional(pool)
                .await?;

        if let Some((token,)) = row {
            debug!("loaded Shopify OAuth token from database");
            *self.inner.token.write().await = Some(SecretString::from(token));
        } else {
            warn!(
                "no Shopify OAuth token found for store {}",
                self.inner.store
            );
        }

        Ok(())
    }

    /// Execute a raw GraphQL query and return the `data` field.
    pub async fn graphql(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<serde_json::Value, ShopifyError> {
        let token = self
            .inner
            .token
            .read()
            .await
            .as_ref()
            .map(|t| t.expose_secret().to_string())
            .ok_or(ShopifyError::NoToken)?;

        let endpoint = format!(
            "https://{}/admin/api/{}/graphql.json",
            self.inner.store, self.inner.api_version
        );

        let body = serde_json::json!({
            "query": query,
            "variables": variables,
        });

        let response: serde_json::Value = self
            .inner
            .client
            .post(&endpoint)
            .header("X-Shopify-Access-Token", &token)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        // Check for GraphQL errors
        if let Some(errors) = response.get("errors").and_then(|e| e.as_array())
            && !errors.is_empty()
        {
            let messages: Vec<&str> = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .collect();
            return Err(ShopifyError::GraphQL(messages.join("; ")));
        }

        response
            .get("data")
            .cloned()
            .ok_or_else(|| ShopifyError::GraphQL("no data in response".to_string()))
    }
}

impl std::fmt::Debug for ShopifyClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShopifyClient")
            .field("store", &self.inner.store)
            .finish_non_exhaustive()
    }
}

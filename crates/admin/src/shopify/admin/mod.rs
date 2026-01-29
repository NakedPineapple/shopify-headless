//! Shopify Admin API GraphQL client with OAuth authentication.
//!
//! This module provides a type-safe client for interacting with the
//! Shopify Admin API using GraphQL. Requires OAuth authentication.

use std::sync::Arc;

use graphql_client::GraphQLQuery;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::sync::RwLock;

use crate::config::ShopifyAdminConfig;

use super::types::{Customer, Payout};
use super::{AdminShopifyError, GraphQLError, GraphQLErrorLocation};

// Domain-specific operations split into separate modules
mod analytics;
mod collections;
mod conversions;
mod customers;
mod discounts;
mod finance;
mod fulfillment;
mod gift_cards;
mod inventory;
mod media;
mod order_editing;
mod orders;
mod products;
pub mod queries;

/// OAuth token for Admin API access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    /// The access token for API calls
    pub access_token: String,
    /// Granted scopes
    pub scope: String,
    /// Unix timestamp when token was obtained
    pub obtained_at: i64,
    /// Associated shop domain
    pub shop: String,
}

/// Input for updating a product.
///
/// All fields are optional - only provided fields will be updated.
#[derive(Debug, Default)]
pub struct ProductUpdateInput<'a> {
    /// New product title.
    pub title: Option<&'a str>,
    /// New product description (HTML).
    pub description_html: Option<&'a str>,
    /// New vendor name.
    pub vendor: Option<&'a str>,
    /// New product type.
    pub product_type: Option<&'a str>,
    /// New tags (replaces existing tags).
    pub tags: Option<Vec<String>>,
    /// New status ("ACTIVE", "DRAFT", or "ARCHIVED").
    pub status: Option<&'a str>,
}

/// Input for creating a discount code.
#[derive(Debug)]
pub struct DiscountCreateInput<'a> {
    /// Internal discount title.
    pub title: &'a str,
    /// Customer-facing discount code.
    pub code: &'a str,
    /// Discount percentage (0.0-1.0) - mutually exclusive with `amount`.
    pub percentage: Option<f64>,
    /// Fixed discount amount (amount, `currency_code`) - mutually exclusive with `percentage`.
    pub amount: Option<(&'a str, &'a str)>,
    /// When the discount becomes active (ISO 8601 datetime).
    pub starts_at: &'a str,
    /// When the discount expires (optional).
    pub ends_at: Option<&'a str>,
    /// Maximum number of uses (optional).
    pub usage_limit: Option<i64>,
}

/// Input for updating a discount code.
#[derive(Debug, Default)]
pub struct DiscountUpdateInput<'a> {
    /// New title (optional).
    pub title: Option<&'a str>,
    /// New start date (optional).
    pub starts_at: Option<&'a str>,
    /// New end date (optional).
    pub ends_at: Option<&'a str>,
}

/// Shopify Admin API GraphQL client.
///
/// Provides type-safe access to the Admin API for managing products,
/// orders, customers, and inventory. Uses OAuth for authentication.
///
/// # Security
///
/// This client uses OAuth credentials which have HIGH PRIVILEGE access
/// to the store. Only use on Tailscale-protected infrastructure.
#[derive(Clone)]
pub struct AdminClient {
    inner: Arc<AdminClientInner>,
}

struct AdminClientInner {
    client: reqwest::Client,
    store: String,
    api_version: String,
    client_id: String,
    client_secret: String,
    /// In-memory token cache (persisted externally via `set_token`/`get_token`)
    token: RwLock<Option<OAuthToken>>,
}

/// GraphQL response wrapper.
#[derive(Debug, Deserialize)]
struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLErrorResponse>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLErrorResponse {
    message: String,
    #[serde(default)]
    locations: Vec<GraphQLErrorLocationResponse>,
    #[serde(default)]
    path: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct GraphQLErrorLocationResponse {
    line: i64,
    column: i64,
}

/// OAuth token response from Shopify.
#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    scope: String,
}

/// Sort customers by a given sort key (for client-side sorting).
///
/// This is used for sort keys not supported by the Shopify API.
fn sort_customers(
    customers: &mut [Customer],
    sort_key: super::types::CustomerSortKey,
    reverse: bool,
) {
    use super::types::CustomerSortKey;

    customers.sort_by(|a, b| {
        let cmp = match sort_key {
            CustomerSortKey::AmountSpent => {
                let a_spent: f64 = a.total_spent.amount.parse().unwrap_or(0.0);
                let b_spent: f64 = b.total_spent.amount.parse().unwrap_or(0.0);
                a_spent
                    .partial_cmp(&b_spent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
            CustomerSortKey::OrdersCount => a.orders_count.cmp(&b.orders_count),
            // These are handled by Shopify API, but include for completeness
            CustomerSortKey::Name => a.display_name.cmp(&b.display_name),
            CustomerSortKey::CreatedAt => a.created_at.cmp(&b.created_at),
            CustomerSortKey::UpdatedAt => a.updated_at.cmp(&b.updated_at),
            CustomerSortKey::Id => a.id.cmp(&b.id),
            CustomerSortKey::Location => {
                let a_loc = a
                    .default_address
                    .as_ref()
                    .and_then(|addr| addr.city.as_ref());
                let b_loc = b
                    .default_address
                    .as_ref()
                    .and_then(|addr| addr.city.as_ref());
                a_loc.cmp(&b_loc)
            }
            CustomerSortKey::Relevance => std::cmp::Ordering::Equal, // Can't sort by relevance client-side
        };

        if reverse { cmp.reverse() } else { cmp }
    });
}

/// Sort payouts by a given sort key (for client-side sorting).
///
/// This is used for sort keys not supported by the Shopify API.
fn sort_payouts(payouts: &mut [Payout], sort_key: super::types::PayoutSortKey, reverse: bool) {
    use super::types::PayoutSortKey;

    payouts.sort_by(|a, b| {
        let cmp = match sort_key {
            // TransactionType requires client-side sorting
            // Note: Payout struct doesn't include transaction_type, this is a no-op
            PayoutSortKey::TransactionType => std::cmp::Ordering::Equal,
            // These are handled by Shopify API, but include for completeness
            PayoutSortKey::IssuedAt => a.issued_at.cmp(&b.issued_at),
            PayoutSortKey::Status => {
                // Convert status to string for comparison
                format!("{:?}", a.status).cmp(&format!("{:?}", b.status))
            }
            PayoutSortKey::Amount => {
                let a_amt: f64 = a.net.amount.parse().unwrap_or(0.0);
                let b_amt: f64 = b.net.amount.parse().unwrap_or(0.0);
                a_amt
                    .partial_cmp(&b_amt)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
            PayoutSortKey::ChargeGross | PayoutSortKey::FeeAmount => {
                // These require PayoutDetail, not available in list view
                std::cmp::Ordering::Equal
            }
            PayoutSortKey::Id => a.id.cmp(&b.id),
        };

        if reverse { cmp.reverse() } else { cmp }
    });
}

impl AdminClient {
    /// Create a new Admin API client.
    ///
    /// # Arguments
    ///
    /// * `config` - Shopify Admin API configuration
    #[must_use]
    pub fn new(config: &ShopifyAdminConfig) -> Self {
        let client = reqwest::Client::new();

        Self {
            inner: Arc::new(AdminClientInner {
                client,
                store: config.store.clone(),
                api_version: config.api_version.clone(),
                client_id: config.client_id.clone(),
                client_secret: config.client_secret.expose_secret().to_string(),
                token: RwLock::new(None),
            }),
        }
    }

    /// Get the store domain.
    #[must_use]
    pub fn store(&self) -> &str {
        &self.inner.store
    }

    /// Get the client ID.
    #[must_use]
    pub fn client_id(&self) -> &str {
        &self.inner.client_id
    }

    /// Get the client secret (for HMAC verification).
    #[must_use]
    pub fn client_secret(&self) -> &str {
        &self.inner.client_secret
    }

    // =========================================================================
    // OAuth Flow
    // =========================================================================

    /// Generate the OAuth authorization URL.
    ///
    /// Redirect the user to this URL to begin the OAuth flow.
    #[must_use]
    pub fn authorization_url(&self, redirect_uri: &str, scopes: &[&str], state: &str) -> String {
        let scope = scopes.join(",");
        format!(
            "https://{}/admin/oauth/authorize?client_id={}&scope={}&redirect_uri={}&state={}",
            self.inner.store,
            urlencoding::encode(&self.inner.client_id),
            urlencoding::encode(&scope),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(state)
        )
    }

    /// Exchange an authorization code for an access token.
    ///
    /// Call this in your OAuth callback handler after the user authorizes.
    ///
    /// # Errors
    ///
    /// Returns `AdminShopifyError::OAuth` if the token exchange fails.
    /// Returns `AdminShopifyError::Http` if the HTTP request fails.
    pub async fn exchange_code(&self, code: &str) -> Result<OAuthToken, AdminShopifyError> {
        let url = format!("https://{}/admin/oauth/access_token", self.inner.store);

        let params = [
            ("client_id", self.inner.client_id.as_str()),
            ("client_secret", self.inner.client_secret.as_str()),
            ("code", code),
        ];

        let response = self.inner.client.post(&url).form(&params).send().await?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(AdminShopifyError::OAuth(format!(
                "Token exchange failed: {text}"
            )));
        }

        let token_response: OAuthTokenResponse = response.json().await?;

        let token = OAuthToken {
            access_token: token_response.access_token,
            scope: token_response.scope,
            obtained_at: chrono::Utc::now().timestamp(),
            shop: self.inner.store.clone(),
        };

        // Cache the token in memory
        *self.inner.token.write().await = Some(token.clone());

        Ok(token)
    }

    /// Set the access token directly (for loading from storage).
    pub async fn set_token(&self, token: OAuthToken) {
        *self.inner.token.write().await = Some(token);
    }

    /// Get the current token (if set).
    pub async fn get_token(&self) -> Option<OAuthToken> {
        self.inner.token.read().await.clone()
    }

    /// Check if we have a valid token.
    pub async fn has_token(&self) -> bool {
        self.inner.token.read().await.is_some()
    }

    /// Clear the cached token.
    pub async fn clear_token(&self) {
        *self.inner.token.write().await = None;
    }

    /// Get the current access token string.
    async fn get_access_token(&self) -> Result<String, AdminShopifyError> {
        let token = self.inner.token.read().await;
        token
            .as_ref()
            .map(|t| t.access_token.clone())
            .ok_or(AdminShopifyError::NoAccessToken)
    }

    // =========================================================================
    // GraphQL Execution
    // =========================================================================

    /// Execute a GraphQL query.
    async fn execute<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<Q::ResponseData, AdminShopifyError>
    where
        Q::ResponseData: DeserializeOwned,
    {
        let access_token = self.get_access_token().await?;
        let endpoint = format!(
            "https://{}/admin/api/{}/graphql.json",
            self.inner.store, self.inner.api_version
        );

        let body = Q::build_query(variables);

        let response = self
            .inner
            .client
            .post(&endpoint)
            .header("X-Shopify-Access-Token", &access_token)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        // Check for rate limiting
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(60);
            return Err(AdminShopifyError::RateLimited(retry_after));
        }

        // Check for unauthorized
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AdminShopifyError::Unauthorized(
                "Invalid or expired access token".to_string(),
            ));
        }

        let graphql_response: GraphQLResponse<Q::ResponseData> = response.json().await?;

        // Check for GraphQL errors
        if let Some(errors) = graphql_response.errors
            && !errors.is_empty()
        {
            let converted_errors: Vec<GraphQLError> = errors
                .into_iter()
                .map(|e| GraphQLError {
                    message: e.message,
                    locations: e
                        .locations
                        .into_iter()
                        .map(|l| GraphQLErrorLocation {
                            line: l.line,
                            column: l.column,
                        })
                        .collect(),
                    path: e.path,
                })
                .collect();
            return Err(AdminShopifyError::GraphQL(converted_errors));
        }

        graphql_response.data.ok_or_else(|| {
            AdminShopifyError::GraphQL(vec![GraphQLError {
                message: "No data in response".to_string(),
                locations: vec![],
                path: vec![],
            }])
        })
    }

    /// Execute a raw GraphQL query with JSON body.
    ///
    /// This is used for mutations that need dynamic field handling
    /// or when the graphql-client codegen doesn't fit the use case.
    async fn execute_raw_graphql(
        &self,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, AdminShopifyError> {
        let access_token = self.get_access_token().await?;
        let endpoint = format!(
            "https://{}/admin/api/{}/graphql.json",
            self.inner.store, self.inner.api_version
        );

        let response: serde_json::Value = self
            .inner
            .client
            .post(&endpoint)
            .header("X-Shopify-Access-Token", &access_token)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        // Check for top-level GraphQL errors
        if let Some(errors) = response.get("errors").and_then(|e| e.as_array())
            && !errors.is_empty()
        {
            return Err(AdminShopifyError::GraphQL(
                errors
                    .iter()
                    .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                    .map(|msg| GraphQLError {
                        message: msg.to_string(),
                        locations: vec![],
                        path: vec![],
                    })
                    .collect(),
            ));
        }

        response.get("data").cloned().ok_or_else(|| {
            AdminShopifyError::GraphQL(vec![GraphQLError {
                message: "No data in response".to_string(),
                locations: vec![],
                path: vec![],
            }])
        })
    }
}

//! Shopify Admin API GraphQL client with OAuth authentication.
//!
//! This module provides a type-safe client for interacting with the
//! Shopify Admin API using GraphQL. Requires OAuth authentication.

use std::sync::Arc;

use graphql_client::GraphQLQuery;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::sync::RwLock;
use tracing::instrument;

use crate::config::ShopifyAdminConfig;

use super::{
    AdminShopifyError, GraphQLError, GraphQLErrorLocation,
    types::{
        AdminProduct, AdminProductConnection, AdminProductVariant, Customer, CustomerConnection,
        Image, InventoryLevel, InventoryLevelConnection, Money, Order, OrderConnection, PageInfo,
    },
};

mod conversions;
pub mod queries;

use conversions::{
    convert_customer, convert_customer_connection, convert_inventory_level_connection,
    convert_order, convert_order_connection, convert_product, convert_product_connection,
};
use queries::{
    GetCustomer, GetCustomers, GetInventoryLevels, GetOrder, GetOrders, GetProduct, GetProducts,
    InventoryAdjustQuantities, InventorySetQuantities,
};

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

    // =========================================================================
    // Product methods
    // =========================================================================

    /// Get a product by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify product ID (e.g., `gid://shopify/Product/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self), fields(product_id = %id))]
    pub async fn get_product(&self, id: &str) -> Result<Option<AdminProduct>, AdminShopifyError> {
        let variables = queries::get_product::Variables {
            id: id.to_string(),
            media_count: Some(10),
            variant_count: Some(50),
        };

        let response = self.execute::<GetProduct>(variables).await?;

        Ok(response.product.map(convert_product))
    }

    /// Get a paginated list of products.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of products to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self))]
    pub async fn get_products(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
    ) -> Result<AdminProductConnection, AdminShopifyError> {
        let variables = queries::get_products::Variables {
            first: Some(first),
            after,
            query,
            sort_key: None,
            reverse: Some(false),
        };

        let response = self.execute::<GetProducts>(variables).await?;

        Ok(convert_product_connection(response.products))
    }

    // =========================================================================
    // Order methods
    // =========================================================================

    /// Get an order by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID (e.g., `gid://shopify/Order/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn get_order(&self, id: &str) -> Result<Option<Order>, AdminShopifyError> {
        let variables = queries::get_order::Variables {
            id: id.to_string(),
            line_item_count: Some(50),
            fulfillment_count: Some(10),
        };

        let response = self.execute::<GetOrder>(variables).await?;

        Ok(response.order.map(convert_order))
    }

    /// Get a paginated list of orders.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of orders to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self))]
    pub async fn get_orders(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
    ) -> Result<OrderConnection, AdminShopifyError> {
        let variables = queries::get_orders::Variables {
            first: Some(first),
            after,
            query,
            sort_key: None,
            reverse: Some(false),
        };

        let response = self.execute::<GetOrders>(variables).await?;

        Ok(convert_order_connection(response.orders))
    }

    // =========================================================================
    // Customer methods
    // =========================================================================

    /// Get a customer by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify customer ID (e.g., `gid://shopify/Customer/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self), fields(customer_id = %id))]
    pub async fn get_customer(&self, id: &str) -> Result<Option<Customer>, AdminShopifyError> {
        let variables = queries::get_customer::Variables {
            id: id.to_string(),
            address_count: Some(10),
        };

        let response = self.execute::<GetCustomer>(variables).await?;

        Ok(response.customer.map(convert_customer))
    }

    /// Get a paginated list of customers.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of customers to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self))]
    pub async fn get_customers(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
    ) -> Result<CustomerConnection, AdminShopifyError> {
        let variables = queries::get_customers::Variables {
            first: Some(first),
            after,
            query,
            sort_key: None,
            reverse: Some(false),
        };

        let response = self.execute::<GetCustomers>(variables).await?;

        Ok(convert_customer_connection(response.customers))
    }

    // =========================================================================
    // Inventory methods
    // =========================================================================

    /// Get inventory levels at a location.
    ///
    /// # Arguments
    ///
    /// * `location_id` - Shopify location ID (e.g., `gid://shopify/Location/123`)
    /// * `first` - Number of inventory levels to return
    /// * `after` - Cursor for pagination
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or the location is not found.
    #[instrument(skip(self), fields(location_id = %location_id))]
    pub async fn get_inventory_levels(
        &self,
        location_id: &str,
        first: i64,
        after: Option<String>,
    ) -> Result<InventoryLevelConnection, AdminShopifyError> {
        let variables = queries::get_inventory_levels::Variables {
            location_id: location_id.to_string(),
            first: Some(first),
            after,
        };

        let response = self.execute::<GetInventoryLevels>(variables).await?;

        response
            .location
            .map(convert_inventory_level_connection)
            .ok_or_else(|| AdminShopifyError::NotFound(format!("Location {location_id} not found")))
    }

    /// Adjust inventory quantity (delta adjustment).
    ///
    /// # Arguments
    ///
    /// * `inventory_item_id` - Shopify inventory item ID
    /// * `location_id` - Shopify location ID
    /// * `delta` - Amount to adjust (positive or negative)
    /// * `reason` - Optional reason for adjustment
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(inventory_item_id = %inventory_item_id, location_id = %location_id, delta = %delta))]
    pub async fn adjust_inventory(
        &self,
        inventory_item_id: &str,
        location_id: &str,
        delta: i64,
        reason: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        use queries::inventory_adjust_quantities::{
            InventoryAdjustQuantitiesInput, InventoryChangeInput,
        };

        let variables = queries::inventory_adjust_quantities::Variables {
            input: InventoryAdjustQuantitiesInput {
                name: "available".to_string(),
                reason: reason.unwrap_or("Manual adjustment").to_string(),
                reference_document_uri: None,
                changes: vec![InventoryChangeInput {
                    inventory_item_id: inventory_item_id.to_string(),
                    location_id: location_id.to_string(),
                    delta,
                    change_from_quantity: None,
                    ledger_document_uri: None,
                }],
            },
        };

        let response = self.execute::<InventoryAdjustQuantities>(variables).await?;

        // Check for user errors
        if let Some(payload) = response.inventory_adjust_quantities
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| e.message.clone())
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Set inventory quantity to an absolute value.
    ///
    /// # Arguments
    ///
    /// * `inventory_item_id` - Shopify inventory item ID
    /// * `location_id` - Shopify location ID
    /// * `quantity` - Quantity to set
    /// * `reason` - Optional reason for adjustment
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(inventory_item_id = %inventory_item_id, location_id = %location_id, quantity = %quantity))]
    pub async fn set_inventory(
        &self,
        inventory_item_id: &str,
        location_id: &str,
        quantity: i64,
        reason: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        use queries::inventory_set_quantities::{
            InventoryQuantityInput, InventorySetQuantitiesInput,
        };

        let variables = queries::inventory_set_quantities::Variables {
            input: InventorySetQuantitiesInput {
                name: "on_hand".to_string(),
                reason: reason.unwrap_or("Manual adjustment").to_string(),
                reference_document_uri: None,
                quantities: vec![InventoryQuantityInput {
                    inventory_item_id: inventory_item_id.to_string(),
                    location_id: location_id.to_string(),
                    quantity,
                    change_from_quantity: None,
                }],
            },
        };

        let response = self.execute::<InventorySetQuantities>(variables).await?;

        // Check for user errors
        if let Some(payload) = response.inventory_set_quantities
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| e.message.clone())
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_client_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<AdminClient>();
    }

    #[test]
    fn test_admin_client_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AdminClient>();
    }
}

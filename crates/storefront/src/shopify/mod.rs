//! Shopify Storefront and Customer Account API clients.
//!
//! # Architecture
//!
//! - Uses `graphql-client` crate for type-safe GraphQL queries
//! - Shopify is source of truth - NO local sync, direct API calls
//! - In-memory caching via `moka` for API responses (5 minute TTL)
//!
//! # APIs
//!
//! ## Storefront API
//! - Products, collections, cart operations
//! - Public access token for client-side operations
//! - Private access token for server-side operations
//!
//! ## Customer Account API
//! - OAuth authentication flow
//! - Customer data, order history
//! - Requires customer consent
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use graphql_client::{GraphQLQuery, Response};
//! use moka::future::Cache;
//! use std::time::Duration;
//!
//! // Generated from graphql/storefront/queries/products.graphql
//! #[derive(GraphQLQuery)]
//! #[graphql(
//!     schema_path = "graphql/storefront/schema.json",
//!     query_path = "graphql/storefront/queries/products.graphql",
//!     response_derives = "Debug, Clone, Serialize"
//! )]
//! pub struct GetProductByHandle;
//!
//! pub struct StorefrontClient {
//!     client: reqwest::Client,
//!     store: String,
//!     api_version: String,
//!     access_token: String,
//!     cache: Cache<String, serde_json::Value>,
//! }
//!
//! impl StorefrontClient {
//!     pub fn new(config: &ShopifyStorefrontConfig) -> Self {
//!         let cache = Cache::builder()
//!             .max_capacity(1000)
//!             .time_to_live(Duration::from_secs(300)) // 5 minutes
//!             .build();
//!
//!         Self {
//!             client: reqwest::Client::new(),
//!             store: config.store.clone(),
//!             api_version: config.api_version.clone(),
//!             access_token: config.storefront_private_token.clone(),
//!             cache,
//!         }
//!     }
//!
//!     fn endpoint(&self) -> String {
//!         format!(
//!             "https://{}/api/{}/graphql.json",
//!             self.store, self.api_version
//!         )
//!     }
//!
//!     pub async fn get_product_by_handle(&self, handle: &str) -> Result<Product, ShopifyError> {
//!         let cache_key = format!("product:{}", handle);
//!
//!         // Check cache first
//!         if let Some(cached) = self.cache.get(&cache_key).await {
//!             return Ok(serde_json::from_value(cached)?);
//!         }
//!
//!         // Make GraphQL request
//!         let variables = get_product_by_handle::Variables {
//!             handle: handle.to_string(),
//!         };
//!         let request_body = GetProductByHandle::build_query(variables);
//!
//!         let response = self
//!             .client
//!             .post(&self.endpoint())
//!             .header("X-Shopify-Storefront-Access-Token", &self.access_token)
//!             .json(&request_body)
//!             .send()
//!             .await?
//!             .json::<Response<get_product_by_handle::ResponseData>>()
//!             .await?;
//!
//!         // Cache and return
//!         let product = response.data?.product?;
//!         self.cache.insert(cache_key, serde_json::to_value(&product)?).await;
//!         Ok(product.into())
//!     }
//!
//!     // Cart operations
//!     pub async fn create_cart(&self) -> Result<Cart, ShopifyError> { ... }
//!     pub async fn add_to_cart(&self, cart_id: &str, lines: Vec<CartLine>) -> Result<Cart, ShopifyError> { ... }
//!     pub async fn update_cart(&self, cart_id: &str, lines: Vec<CartLineUpdate>) -> Result<Cart, ShopifyError> { ... }
//!     pub async fn get_cart(&self, cart_id: &str) -> Result<Cart, ShopifyError> { ... }
//! }
//!
//! pub struct CustomerClient {
//!     client: reqwest::Client,
//!     store: String,
//!     client_id: String,
//!     client_secret: String,
//! }
//!
//! impl CustomerClient {
//!     /// Generate OAuth authorization URL.
//!     pub fn authorization_url(&self, redirect_uri: &str, state: &str) -> String { ... }
//!
//!     /// Exchange authorization code for tokens.
//!     pub async fn exchange_code(&self, code: &str, redirect_uri: &str) -> Result<TokenResponse, ShopifyError> { ... }
//!
//!     /// Get customer data using access token.
//!     pub async fn get_customer(&self, access_token: &str) -> Result<Customer, ShopifyError> { ... }
//!
//!     /// Get customer's order history.
//!     pub async fn get_orders(&self, access_token: &str) -> Result<Vec<Order>, ShopifyError> { ... }
//! }
//!
//! #[derive(Debug, thiserror::Error)]
//! pub enum ShopifyError {
//!     #[error("HTTP error: {0}")]
//!     Http(#[from] reqwest::Error),
//!
//!     #[error("GraphQL error: {0}")]
//!     GraphQL(String),
//!
//!     #[error("Not found")]
//!     NotFound,
//!
//!     #[error("Rate limited")]
//!     RateLimited,
//! }
//! ```
//!
//! # GraphQL Queries to Create
//!
//! - `queries/products.graphql` - `GetProductByHandle`, `GetProducts`
//! - `queries/collections.graphql` - `GetCollectionByHandle`, `GetCollections`
//! - `queries/cart.graphql` - `CreateCart`, `AddToCart`, `UpdateCart`, `GetCart`

// TODO: Implement Storefront and Customer API clients

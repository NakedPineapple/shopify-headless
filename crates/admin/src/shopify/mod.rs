//! Shopify Admin API client (HIGH PRIVILEGE - Tailscale only).
//!
//! # Security
//!
//! **CRITICAL: This crate contains the high-privilege Shopify Admin API token.**
//!
//! It should ONLY run on Tailscale-protected infrastructure with MDM verification.
//! The Admin API has full access to:
//! - Products, variants, inventory
//! - Orders, fulfillments, refunds
//! - Customers, customer data
//! - Discounts, price rules
//! - Shop settings
//!
//! # Architecture
//!
//! - Uses `graphql-client` crate for type-safe GraphQL queries
//! - Direct API calls to Shopify (no local database sync)
//! - Rate limiting handled automatically
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use graphql_client::{GraphQLQuery, Response};
//!
//! // Generated from graphql/admin/queries/products.graphql
//! #[derive(GraphQLQuery)]
//! #[graphql(
//!     schema_path = "graphql/admin/schema.json",
//!     query_path = "graphql/admin/queries/products.graphql",
//!     response_derives = "Debug, Clone, Serialize"
//! )]
//! pub struct GetProducts;
//!
//! pub struct AdminClient {
//!     client: reqwest::Client,
//!     store: String,
//!     api_version: String,
//!     access_token: String,  // HIGH PRIVILEGE TOKEN
//! }
//!
//! impl AdminClient {
//!     pub fn new(config: &ShopifyAdminConfig) -> Self {
//!         Self {
//!             client: reqwest::Client::new(),
//!             store: config.store.clone(),
//!             api_version: config.api_version.clone(),
//!             access_token: config.access_token.clone(),
//!         }
//!     }
//!
//!     fn endpoint(&self) -> String {
//!         format!(
//!             "https://{}/admin/api/{}/graphql.json",
//!             self.store, self.api_version
//!         )
//!     }
//!
//!     /// Get products with pagination.
//!     pub async fn get_products(&self, first: i64, after: Option<&str>) -> Result<ProductConnection, ShopifyError> {
//!         let variables = get_products::Variables {
//!             first,
//!             after: after.map(String::from),
//!         };
//!         let request_body = GetProducts::build_query(variables);
//!
//!         let response = self
//!             .client
//!             .post(&self.endpoint())
//!             .header("X-Shopify-Access-Token", &self.access_token)
//!             .json(&request_body)
//!             .send()
//!             .await?
//!             .json::<Response<get_products::ResponseData>>()
//!             .await?;
//!
//!         Ok(response.data?.products.into())
//!     }
//!
//!     /// Get orders with pagination.
//!     pub async fn get_orders(&self, first: i64, after: Option<&str>) -> Result<OrderConnection, ShopifyError> { ... }
//!
//!     /// Get a single order by ID.
//!     pub async fn get_order(&self, id: &str) -> Result<Order, ShopifyError> { ... }
//!
//!     /// Get customers with pagination.
//!     pub async fn get_customers(&self, first: i64, after: Option<&str>) -> Result<CustomerConnection, ShopifyError> { ... }
//!
//!     /// Get inventory levels for a location.
//!     pub async fn get_inventory(&self, location_id: &str) -> Result<Vec<InventoryLevel>, ShopifyError> { ... }
//!
//!     /// Update inventory quantity.
//!     pub async fn adjust_inventory(&self, inventory_item_id: &str, location_id: &str, delta: i64) -> Result<InventoryLevel, ShopifyError> { ... }
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
//!     #[error("Rate limited, retry after {0} seconds")]
//!     RateLimited(u64),
//!
//!     #[error("Unauthorized")]
//!     Unauthorized,
//! }
//! ```
//!
//! # GraphQL Queries to Create
//!
//! - `queries/products.graphql` - `GetProducts`, `GetProduct`, `UpdateProduct`
//! - `queries/orders.graphql` - `GetOrders`, `GetOrder`, `FulfillOrder`
//! - `queries/customers.graphql` - `GetCustomers`, `GetCustomer`
//! - `queries/inventory.graphql` - `GetInventoryLevels`, `AdjustInventory`

// TODO: Implement Admin API client

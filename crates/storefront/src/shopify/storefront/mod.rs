//! Shopify Storefront API client implementation.
//!
//! Uses `graphql_client` for type-safe queries with `reqwest` 0.13 for HTTP.
//! Caches products and collections using `moka` (5-minute TTL).

mod cache;
mod conversions;
pub mod queries;

// Scalar types must be declared directly in this module (not just re-exported)
// so graphql_client can find them as super::TypeName during macro expansion
// Note: These MUST match the GraphQL schema scalar names exactly (uppercase)
#[allow(clippy::upper_case_acronyms)]
pub type DateTime = String;
#[allow(clippy::upper_case_acronyms)]
pub type Decimal = String;
#[allow(clippy::upper_case_acronyms)]
pub type URL = String;
#[allow(clippy::upper_case_acronyms)]
pub type HTML = String;
#[allow(clippy::upper_case_acronyms)]
pub type Color = String;
#[allow(clippy::upper_case_acronyms)]
pub type JSON = serde_json::Value;
pub type UnsignedInt64 = String;

use std::sync::Arc;
use std::time::Duration;

use graphql_client::{GraphQLQuery, Response};
use moka::future::Cache;
use secrecy::ExposeSecret;
use tracing::{debug, instrument};

use crate::config::ShopifyStorefrontConfig;
use crate::shopify::ShopifyError;
use crate::shopify::types::{
    Cart, CartLineInput, CartLineUpdateInput, CartUserError, Collection, CollectionConnection,
    Product, ProductConnection, ProductRecommendationIntent,
};

use cache::CacheValue;
use conversions::{
    convert_add_user_error, convert_cart, convert_collection, convert_collection_connection,
    convert_discount_user_error, convert_note_user_error, convert_product,
    convert_product_connection, convert_product_recommendation, convert_remove_user_error,
    convert_update_user_error, convert_user_error,
};
use queries::{
    AddToCart, CreateCart, CustomerAccessTokenCreate, CustomerAccessTokenDelete,
    CustomerAccessTokenRenew, CustomerActivateByUrl, CustomerCreate, CustomerRecover,
    CustomerResetByUrl, GetCart, GetCollectionByHandle, GetCollections, GetCustomerByToken,
    GetProductByHandle, GetProductRecommendations, GetProducts, RemoveFromCart,
    UpdateCartDiscountCodes, UpdateCartLines, UpdateCartNote, add_to_cart, create_cart,
    customer_access_token_create, customer_access_token_delete, customer_access_token_renew,
    customer_activate_by_url, customer_create, customer_recover, customer_reset_by_url, get_cart,
    get_collection_by_handle, get_collections, get_customer_by_token, get_product_by_handle,
    get_product_recommendations, get_products, remove_from_cart, update_cart_discount_codes,
    update_cart_lines, update_cart_note,
};

// =============================================================================
// StorefrontClient
// =============================================================================

/// Client for the Shopify Storefront API.
///
/// Provides type-safe access to products, collections, and cart operations.
/// Products and collections are cached for 5 minutes.
#[derive(Clone)]
pub struct StorefrontClient {
    inner: Arc<StorefrontClientInner>,
}

struct StorefrontClientInner {
    client: reqwest::Client,
    endpoint: String,
    access_token: String,
    cache: Cache<String, CacheValue>,
}

impl StorefrontClient {
    /// Create a new Storefront API client.
    #[must_use]
    pub fn new(config: &ShopifyStorefrontConfig) -> Self {
        let cache = Cache::builder()
            .max_capacity(1000)
            .time_to_live(Duration::from_secs(300)) // 5 minutes
            .build();

        let endpoint = format!(
            "https://{}/api/{}/graphql.json",
            config.store, config.api_version
        );

        Self {
            inner: Arc::new(StorefrontClientInner {
                client: reqwest::Client::new(),
                endpoint,
                access_token: config.storefront_private_token.expose_secret().to_string(),
                cache,
            }),
        }
    }

    /// Execute a GraphQL query.
    async fn execute<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<Q::ResponseData, ShopifyError>
    where
        Q::Variables: serde::Serialize,
    {
        let request_body = Q::build_query(variables);

        let response = self
            .inner
            .client
            .post(&self.inner.endpoint)
            // Private access tokens use a different header than public tokens
            // See: https://shopify.dev/docs/storefronts/headless/building-with-the-storefront-api/getting-started
            .header("Shopify-Storefront-Private-Token", &self.inner.access_token)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();

        // Check for rate limiting
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(1);
            return Err(ShopifyError::RateLimited(retry_after));
        }

        // Get response body as text first for better error diagnostics
        let response_text = response.text().await?;

        // Check for non-success status codes
        if !status.is_success() {
            tracing::error!(
                status = %status,
                body = %response_text.chars().take(500).collect::<String>(),
                "Shopify API returned non-success status"
            );
            return Err(ShopifyError::GraphQL(vec![super::GraphQLError {
                message: format!(
                    "HTTP {status}: {}",
                    response_text.chars().take(200).collect::<String>()
                ),
                locations: vec![],
                path: vec![],
            }]));
        }

        // Parse the response
        let response: Response<Q::ResponseData> = match serde_json::from_str(&response_text) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(
                    error = %e,
                    body = %response_text.chars().take(500).collect::<String>(),
                    "Failed to parse Shopify GraphQL response"
                );
                return Err(ShopifyError::Parse(e));
            }
        };

        // Check for GraphQL errors
        if let Some(errors) = response.errors
            && !errors.is_empty()
        {
            // Log the raw errors for debugging
            tracing::debug!(
                errors = ?errors,
                "GraphQL errors in response"
            );

            return Err(ShopifyError::GraphQL(
                errors
                    .into_iter()
                    .map(|e| super::GraphQLError {
                        message: e.message,
                        locations: e.locations.map_or_else(Vec::new, |locs| {
                            locs.into_iter()
                                .map(|l| super::GraphQLErrorLocation {
                                    line: i64::from(l.line),
                                    column: i64::from(l.column),
                                })
                                .collect()
                        }),
                        path: e.path.map_or_else(Vec::new, |p| {
                            p.into_iter()
                                .map(|fragment| match fragment {
                                    graphql_client::PathFragment::Key(s) => {
                                        serde_json::Value::String(s)
                                    }
                                    graphql_client::PathFragment::Index(i) => {
                                        serde_json::Value::Number(i.into())
                                    }
                                })
                                .collect()
                        }),
                    })
                    .collect(),
            ));
        }

        response.data.ok_or_else(|| {
            tracing::error!(
                body = %response_text.chars().take(500).collect::<String>(),
                "Shopify GraphQL response has no data and no errors"
            );
            ShopifyError::GraphQL(vec![super::GraphQLError {
                message: "No data in response".to_string(),
                locations: vec![],
                path: vec![],
            }])
        })
    }

    // =========================================================================
    // Product Methods
    // =========================================================================

    /// Get a product by its handle.
    ///
    /// # Errors
    ///
    /// Returns an error if the product is not found or the API request fails.
    #[instrument(skip(self), fields(handle = %handle))]
    pub async fn get_product_by_handle(&self, handle: &str) -> Result<Product, ShopifyError> {
        let cache_key = format!("product:{handle}");

        // Check cache
        if let Some(CacheValue::Product(product)) = self.inner.cache.get(&cache_key).await {
            debug!("Cache hit for product");
            return Ok(*product);
        }

        let variables = get_product_by_handle::Variables {
            handle: handle.to_string(),
            image_count: Some(10),
            variant_count: Some(50),
        };

        let data = self.execute::<GetProductByHandle>(variables).await?;

        let product_data = data
            .product
            .ok_or_else(|| ShopifyError::NotFound(format!("Product not found: {handle}")))?;

        let product = convert_product(product_data);

        // Cache the result
        self.inner
            .cache
            .insert(cache_key, CacheValue::Product(Box::new(product.clone())))
            .await;

        Ok(product)
    }

    /// Get a paginated list of products.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_products(
        &self,
        first: Option<i64>,
        after: Option<String>,
        query: Option<String>,
        sort_key: Option<get_products::ProductSortKeys>,
        reverse: Option<bool>,
    ) -> Result<ProductConnection, ShopifyError> {
        let cache_key = format!("products:{}:{:?}", after.as_deref().unwrap_or(""), query);

        // Check cache (only for default queries without search)
        if query.is_none()
            && let Some(CacheValue::Products(products)) = self.inner.cache.get(&cache_key).await
        {
            debug!("Cache hit for products");
            return Ok(products);
        }

        let variables = get_products::Variables {
            first,
            after: after.clone(),
            query: query.clone(),
            sort_key,
            reverse,
        };

        let data = self.execute::<GetProducts>(variables).await?;

        let connection = convert_product_connection(data.products);

        // Cache if not a search query
        if query.is_none() {
            self.inner
                .cache
                .insert(cache_key, CacheValue::Products(connection.clone()))
                .await;
        }

        Ok(connection)
    }

    /// Get product recommendations.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self), fields(product_id = %product_id))]
    pub async fn get_product_recommendations(
        &self,
        product_id: &str,
        intent: Option<ProductRecommendationIntent>,
    ) -> Result<Vec<Product>, ShopifyError> {
        let variables = get_product_recommendations::Variables {
            product_id: product_id.to_string(),
            intent: intent.map(|i| match i {
                ProductRecommendationIntent::Related => {
                    get_product_recommendations::ProductRecommendationIntent::RELATED
                }
                ProductRecommendationIntent::Complementary => {
                    get_product_recommendations::ProductRecommendationIntent::COMPLEMENTARY
                }
            }),
        };

        let data = self.execute::<GetProductRecommendations>(variables).await?;

        let products = data
            .product_recommendations
            .map(|recs| {
                recs.into_iter()
                    .map(convert_product_recommendation)
                    .collect()
            })
            .unwrap_or_default();

        Ok(products)
    }

    // =========================================================================
    // Collection Methods
    // =========================================================================

    /// Get a collection by its handle.
    ///
    /// # Errors
    ///
    /// Returns an error if the collection is not found or the API request fails.
    #[instrument(skip(self, filters), fields(handle = %handle))]
    pub async fn get_collection_by_handle(
        &self,
        handle: &str,
        product_count: Option<i64>,
        after: Option<String>,
        sort_key: Option<get_collection_by_handle::ProductCollectionSortKeys>,
        reverse: Option<bool>,
        filters: Option<Vec<get_collection_by_handle::ProductFilter>>,
    ) -> Result<Collection, ShopifyError> {
        // Include sort params in cache key
        let sort_str = sort_key.as_ref().map_or("default", |k| match k {
            get_collection_by_handle::ProductCollectionSortKeys::BEST_SELLING => "best",
            get_collection_by_handle::ProductCollectionSortKeys::PRICE => "price",
            get_collection_by_handle::ProductCollectionSortKeys::CREATED => "created",
            get_collection_by_handle::ProductCollectionSortKeys::TITLE => "title",
            _ => "other",
        });
        let reverse_str = reverse.unwrap_or(false);
        // Include filter state in cache key
        let filter_str = filters.as_ref().map_or(String::new(), |f| {
            f.iter()
                .map(|filter| {
                    let mut parts = Vec::new();
                    if let Some(avail) = filter.available {
                        parts.push(format!("avail:{avail}"));
                    }
                    if let Some(ref price) = filter.price {
                        if let Some(min) = price.min {
                            parts.push(format!("min:{min}"));
                        }
                        if let Some(max) = price.max {
                            parts.push(format!("max:{max}"));
                        }
                    }
                    parts.join(",")
                })
                .collect::<Vec<_>>()
                .join(";")
        });
        let cache_key = format!(
            "collection:{handle}:{}:{}:{}:{}",
            after.as_deref().unwrap_or(""),
            sort_str,
            reverse_str,
            filter_str
        );

        // Check cache
        if let Some(CacheValue::Collection(collection)) = self.inner.cache.get(&cache_key).await {
            debug!("Cache hit for collection");
            return Ok(*collection);
        }

        let variables = get_collection_by_handle::Variables {
            handle: handle.to_string(),
            product_count,
            after: after.clone(),
            sort_key,
            reverse,
            filters,
        };

        let data = self.execute::<GetCollectionByHandle>(variables).await?;

        let collection_data = data
            .collection
            .ok_or_else(|| ShopifyError::NotFound(format!("Collection not found: {handle}")))?;

        let collection = convert_collection(collection_data);

        // Cache the result
        self.inner
            .cache
            .insert(
                cache_key,
                CacheValue::Collection(Box::new(collection.clone())),
            )
            .await;

        Ok(collection)
    }

    /// Get a paginated list of collections.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_collections(
        &self,
        first: Option<i64>,
        after: Option<String>,
        query: Option<String>,
    ) -> Result<CollectionConnection, ShopifyError> {
        let cache_key = format!("collections:{}:{:?}", after.as_deref().unwrap_or(""), query);

        // Check cache (only for default queries)
        if query.is_none()
            && let Some(CacheValue::Collections(collections)) =
                self.inner.cache.get(&cache_key).await
        {
            debug!("Cache hit for collections");
            return Ok(collections);
        }

        let variables = get_collections::Variables {
            first,
            after: after.clone(),
            query: query.clone(),
            sort_key: None,
            reverse: None,
        };

        let data = self.execute::<GetCollections>(variables).await?;

        let connection = convert_collection_connection(data.collections);

        // Cache if not a search query
        if query.is_none() {
            self.inner
                .cache
                .insert(cache_key, CacheValue::Collections(connection.clone()))
                .await;
        }

        Ok(connection)
    }

    // =========================================================================
    // Cart Methods (not cached - mutable state)
    // =========================================================================

    /// Create a new cart.
    ///
    /// # Errors
    ///
    /// Returns an error if the cart creation fails or user errors are returned.
    #[instrument(skip(self, lines))]
    pub async fn create_cart(
        &self,
        lines: Option<Vec<CartLineInput>>,
        note: Option<String>,
    ) -> Result<Cart, ShopifyError> {
        let variables = create_cart::Variables {
            input: create_cart::CartInput {
                lines: lines.map(|l| {
                    l.into_iter()
                        .map(|line| create_cart::CartLineInput {
                            merchandise_id: line.merchandise_id,
                            quantity: Some(line.quantity),
                            attributes: line.attributes.map(|attrs| {
                                attrs
                                    .into_iter()
                                    .map(|a| create_cart::AttributeInput {
                                        key: a.key,
                                        value: a.value,
                                    })
                                    .collect()
                            }),
                            selling_plan_id: line.selling_plan_id,
                        })
                        .collect()
                }),
                note,
                attributes: None,
                discount_codes: None,
                buyer_identity: None,
                metafields: None,
                delivery: None,
                gift_card_codes: None,
            },
        };

        let data = self.execute::<CreateCart>(variables).await?;

        if let Some(result) = data.cart_create {
            // Check for user errors
            if !result.user_errors.is_empty() {
                return Err(ShopifyError::UserError(
                    result
                        .user_errors
                        .into_iter()
                        .map(|e| convert_user_error(e).message)
                        .collect::<Vec<_>>()
                        .join("; "),
                ));
            }

            if let Some(cart) = result.cart {
                return Ok(convert_cart(cart));
            }
        }

        Err(ShopifyError::GraphQL(vec![super::GraphQLError {
            message: "Failed to create cart".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Get an existing cart.
    ///
    /// # Errors
    ///
    /// Returns an error if the cart is not found or the API request fails.
    #[instrument(skip(self), fields(cart_id = %cart_id))]
    pub async fn get_cart(&self, cart_id: &str) -> Result<Cart, ShopifyError> {
        let variables = get_cart::Variables {
            cart_id: cart_id.to_string(),
        };

        let data = self.execute::<GetCart>(variables).await?;

        data.cart
            .map(convert_cart)
            .ok_or_else(|| ShopifyError::NotFound(format!("Cart not found: {cart_id}")))
    }

    /// Add lines to a cart.
    ///
    /// # Errors
    ///
    /// Returns an error if the cart update fails or user errors are returned.
    #[instrument(skip(self, lines), fields(cart_id = %cart_id))]
    pub async fn add_to_cart(
        &self,
        cart_id: &str,
        lines: Vec<CartLineInput>,
    ) -> Result<Cart, ShopifyError> {
        let variables = add_to_cart::Variables {
            cart_id: cart_id.to_string(),
            lines: lines
                .into_iter()
                .map(|line| add_to_cart::CartLineInput {
                    merchandise_id: line.merchandise_id,
                    quantity: Some(line.quantity),
                    attributes: line.attributes.map(|attrs| {
                        attrs
                            .into_iter()
                            .map(|a| add_to_cart::AttributeInput {
                                key: a.key,
                                value: a.value,
                            })
                            .collect()
                    }),
                    selling_plan_id: line.selling_plan_id,
                })
                .collect(),
        };

        let data = self.execute::<AddToCart>(variables).await?;

        if let Some(result) = data.cart_lines_add {
            if !result.user_errors.is_empty() {
                return Err(ShopifyError::UserError(
                    result
                        .user_errors
                        .into_iter()
                        .map(|e| convert_add_user_error(e).message)
                        .collect::<Vec<_>>()
                        .join("; "),
                ));
            }

            if let Some(cart) = result.cart {
                return Ok(convert_cart(cart));
            }
        }

        Err(ShopifyError::GraphQL(vec![super::GraphQLError {
            message: "Failed to add to cart".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update cart lines.
    ///
    /// # Errors
    ///
    /// Returns an error if the cart update fails or user errors are returned.
    #[instrument(skip(self, lines), fields(cart_id = %cart_id))]
    pub async fn update_cart(
        &self,
        cart_id: &str,
        lines: Vec<CartLineUpdateInput>,
    ) -> Result<Cart, ShopifyError> {
        let variables = update_cart_lines::Variables {
            cart_id: cart_id.to_string(),
            lines: lines
                .into_iter()
                .map(|line| update_cart_lines::CartLineUpdateInput {
                    id: line.id,
                    quantity: line.quantity,
                    merchandise_id: line.merchandise_id,
                    attributes: line.attributes.map(|attrs| {
                        attrs
                            .into_iter()
                            .map(|a| update_cart_lines::AttributeInput {
                                key: a.key,
                                value: a.value,
                            })
                            .collect()
                    }),
                    selling_plan_id: line.selling_plan_id,
                })
                .collect(),
        };

        let data = self.execute::<UpdateCartLines>(variables).await?;

        if let Some(result) = data.cart_lines_update {
            if !result.user_errors.is_empty() {
                return Err(ShopifyError::UserError(
                    result
                        .user_errors
                        .into_iter()
                        .map(|e| convert_update_user_error(e).message)
                        .collect::<Vec<_>>()
                        .join("; "),
                ));
            }

            if let Some(cart) = result.cart {
                return Ok(convert_cart(cart));
            }
        }

        Err(ShopifyError::GraphQL(vec![super::GraphQLError {
            message: "Failed to update cart".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Remove lines from a cart.
    ///
    /// # Errors
    ///
    /// Returns an error if the cart update fails or user errors are returned.
    #[instrument(skip(self, line_ids), fields(cart_id = %cart_id))]
    pub async fn remove_from_cart(
        &self,
        cart_id: &str,
        line_ids: Vec<String>,
    ) -> Result<Cart, ShopifyError> {
        let variables = remove_from_cart::Variables {
            cart_id: cart_id.to_string(),
            line_ids,
        };

        let data = self.execute::<RemoveFromCart>(variables).await?;

        if let Some(result) = data.cart_lines_remove {
            if !result.user_errors.is_empty() {
                return Err(ShopifyError::UserError(
                    result
                        .user_errors
                        .into_iter()
                        .map(|e| convert_remove_user_error(e).message)
                        .collect::<Vec<_>>()
                        .join("; "),
                ));
            }

            if let Some(cart) = result.cart {
                return Ok(convert_cart(cart));
            }
        }

        Err(ShopifyError::GraphQL(vec![super::GraphQLError {
            message: "Failed to remove from cart".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update discount codes on a cart.
    ///
    /// # Errors
    ///
    /// Returns an error if the cart update fails or user errors are returned.
    #[instrument(skip(self, discount_codes), fields(cart_id = %cart_id))]
    pub async fn update_discount_codes(
        &self,
        cart_id: &str,
        discount_codes: Vec<String>,
    ) -> Result<Cart, ShopifyError> {
        let variables = update_cart_discount_codes::Variables {
            cart_id: cart_id.to_string(),
            discount_codes,
        };

        let data = self.execute::<UpdateCartDiscountCodes>(variables).await?;

        if let Some(result) = data.cart_discount_codes_update {
            if !result.user_errors.is_empty() {
                return Err(ShopifyError::UserError(
                    result
                        .user_errors
                        .into_iter()
                        .map(|e| convert_discount_user_error(e).message)
                        .collect::<Vec<_>>()
                        .join("; "),
                ));
            }

            if let Some(cart) = result.cart {
                return Ok(convert_cart(cart));
            }
        }

        Err(ShopifyError::GraphQL(vec![super::GraphQLError {
            message: "Failed to update discount codes".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update cart note.
    ///
    /// # Errors
    ///
    /// Returns an error if the cart update fails or user errors are returned.
    #[instrument(skip(self), fields(cart_id = %cart_id))]
    pub async fn update_cart_note(&self, cart_id: &str, note: &str) -> Result<Cart, ShopifyError> {
        let variables = update_cart_note::Variables {
            cart_id: cart_id.to_string(),
            note: note.to_string(),
        };

        let data = self.execute::<UpdateCartNote>(variables).await?;

        if let Some(result) = data.cart_note_update {
            if !result.user_errors.is_empty() {
                return Err(ShopifyError::UserError(
                    result
                        .user_errors
                        .into_iter()
                        .map(|e| convert_note_user_error(e).message)
                        .collect::<Vec<_>>()
                        .join("; "),
                ));
            }

            if let Some(cart) = result.cart {
                return Ok(convert_cart(cart));
            }
        }

        Err(ShopifyError::GraphQL(vec![super::GraphQLError {
            message: "Failed to update cart note".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    // =========================================================================
    // Cache Management
    // =========================================================================

    /// Invalidate a cached product.
    pub async fn invalidate_product(&self, handle: &str) {
        let cache_key = format!("product:{handle}");
        self.inner.cache.invalidate(&cache_key).await;
    }

    /// Invalidate a cached collection.
    pub async fn invalidate_collection(&self, handle: &str) {
        self.inner
            .cache
            .invalidate(&format!("collection:{handle}:"))
            .await;
    }

    /// Invalidate all cached data.
    pub async fn invalidate_all(&self) {
        self.inner.cache.invalidate_all();
        self.inner.cache.run_pending_tasks().await;
    }

    // =========================================================================
    // Customer Authentication Methods (Storefront API)
    // =========================================================================

    /// Create a new customer account.
    ///
    /// Shopify will automatically send an activation email to the customer.
    /// The customer must click the activation link to set their password.
    ///
    /// # Arguments
    ///
    /// * `email` - Customer's email address
    /// * `password` - Initial password (customer may change via activation email)
    /// * `first_name` - Optional first name
    /// * `last_name` - Optional last name
    /// * `accepts_marketing` - Whether customer accepts marketing emails
    ///
    /// # Errors
    ///
    /// Returns an error if the customer already exists or validation fails.
    #[instrument(skip(self, password), fields(email = %email))]
    pub async fn create_customer(
        &self,
        email: &str,
        password: &str,
        first_name: Option<&str>,
        last_name: Option<&str>,
        accepts_marketing: bool,
    ) -> Result<StorefrontCustomer, ShopifyError> {
        let variables = customer_create::Variables {
            input: customer_create::CustomerCreateInput {
                email: email.to_string(),
                password: password.to_string(),
                first_name: first_name.map(String::from),
                last_name: last_name.map(String::from),
                accepts_marketing: Some(accepts_marketing),
                phone: None,
            },
        };

        let data = self.execute::<CustomerCreate>(variables).await?;

        if let Some(result) = data.customer_create {
            // Check for user errors
            if !result.customer_user_errors.is_empty() {
                let errors: Vec<_> = result
                    .customer_user_errors
                    .iter()
                    .map(|e| e.message.as_str())
                    .collect();
                return Err(ShopifyError::UserError(errors.join("; ")));
            }

            if let Some(customer) = result.customer {
                return Ok(StorefrontCustomer {
                    id: customer.id,
                    email: customer.email,
                    first_name: customer.first_name,
                    last_name: customer.last_name,
                });
            }
        }

        Err(ShopifyError::GraphQL(vec![super::GraphQLError {
            message: "Failed to create customer".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Create an access token for a customer (login).
    ///
    /// # Arguments
    ///
    /// * `email` - Customer's email address
    /// * `password` - Customer's password
    ///
    /// # Errors
    ///
    /// Returns an error if the credentials are invalid.
    #[instrument(skip(self, password), fields(email = %email))]
    pub async fn create_access_token(
        &self,
        email: &str,
        password: &str,
    ) -> Result<StorefrontAccessToken, ShopifyError> {
        let variables = customer_access_token_create::Variables {
            input: customer_access_token_create::CustomerAccessTokenCreateInput {
                email: email.to_string(),
                password: password.to_string(),
            },
        };

        let data = self.execute::<CustomerAccessTokenCreate>(variables).await?;

        if let Some(result) = data.customer_access_token_create {
            // Check for user errors
            if !result.customer_user_errors.is_empty() {
                let errors: Vec<_> = result
                    .customer_user_errors
                    .iter()
                    .map(|e| e.message.as_str())
                    .collect();
                return Err(ShopifyError::UserError(errors.join("; ")));
            }

            if let Some(token) = result.customer_access_token {
                return Ok(StorefrontAccessToken {
                    access_token: token.access_token,
                    expires_at: token.expires_at,
                });
            }
        }

        Err(ShopifyError::UserError(
            "Invalid email or password".to_string(),
        ))
    }

    /// Activate a customer account using the activation URL from Shopify's email.
    ///
    /// # Arguments
    ///
    /// * `activation_url` - The full activation URL from Shopify's email
    /// * `password` - The password the customer wants to set
    ///
    /// # Errors
    ///
    /// Returns an error if the activation URL is invalid or expired.
    #[instrument(skip(self, password))]
    pub async fn activate_customer_by_url(
        &self,
        activation_url: &str,
        password: &str,
    ) -> Result<(StorefrontCustomer, StorefrontAccessToken), ShopifyError> {
        let variables = customer_activate_by_url::Variables {
            activation_url: activation_url.to_string(),
            password: password.to_string(),
        };

        let data = self.execute::<CustomerActivateByUrl>(variables).await?;

        if let Some(result) = data.customer_activate_by_url {
            // Check for user errors
            if !result.customer_user_errors.is_empty() {
                let errors: Vec<_> = result
                    .customer_user_errors
                    .iter()
                    .map(|e| e.message.as_str())
                    .collect();
                return Err(ShopifyError::UserError(errors.join("; ")));
            }

            let customer = result.customer.ok_or_else(|| {
                ShopifyError::GraphQL(vec![super::GraphQLError {
                    message: "No customer returned from activation".to_string(),
                    locations: vec![],
                    path: vec![],
                }])
            })?;

            let token = result.customer_access_token.ok_or_else(|| {
                ShopifyError::GraphQL(vec![super::GraphQLError {
                    message: "No access token returned from activation".to_string(),
                    locations: vec![],
                    path: vec![],
                }])
            })?;

            return Ok((
                StorefrontCustomer {
                    id: customer.id,
                    email: customer.email,
                    first_name: None,
                    last_name: None,
                },
                StorefrontAccessToken {
                    access_token: token.access_token,
                    expires_at: token.expires_at,
                },
            ));
        }

        Err(ShopifyError::GraphQL(vec![super::GraphQLError {
            message: "Failed to activate customer".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Send a password recovery email to a customer.
    ///
    /// Shopify will send the email automatically.
    ///
    /// # Arguments
    ///
    /// * `email` - Customer's email address
    ///
    /// # Errors
    ///
    /// Returns an error if the email is invalid.
    #[instrument(skip(self), fields(email = %email))]
    pub async fn recover_customer(&self, email: &str) -> Result<(), ShopifyError> {
        let variables = customer_recover::Variables {
            email: email.to_string(),
        };

        let data = self.execute::<CustomerRecover>(variables).await?;

        if let Some(result) = data.customer_recover {
            // Check for user errors
            if !result.customer_user_errors.is_empty() {
                let errors: Vec<_> = result
                    .customer_user_errors
                    .iter()
                    .map(|e| e.message.as_str())
                    .collect();
                return Err(ShopifyError::UserError(errors.join("; ")));
            }
        }

        // Success - email sent (or silently ignored if customer doesn't exist for security)
        Ok(())
    }

    /// Reset a customer's password using the reset URL from Shopify's email.
    ///
    /// # Arguments
    ///
    /// * `reset_url` - The full reset URL from Shopify's email
    /// * `password` - The new password
    ///
    /// # Errors
    ///
    /// Returns an error if the reset URL is invalid or expired.
    #[instrument(skip(self, password))]
    pub async fn reset_customer_by_url(
        &self,
        reset_url: &str,
        password: &str,
    ) -> Result<(StorefrontCustomer, StorefrontAccessToken), ShopifyError> {
        let variables = customer_reset_by_url::Variables {
            reset_url: reset_url.to_string(),
            password: password.to_string(),
        };

        let data = self.execute::<CustomerResetByUrl>(variables).await?;

        if let Some(result) = data.customer_reset_by_url {
            // Check for user errors
            if !result.customer_user_errors.is_empty() {
                let errors: Vec<_> = result
                    .customer_user_errors
                    .iter()
                    .map(|e| e.message.as_str())
                    .collect();
                return Err(ShopifyError::UserError(errors.join("; ")));
            }

            let customer = result.customer.ok_or_else(|| {
                ShopifyError::GraphQL(vec![super::GraphQLError {
                    message: "No customer returned from password reset".to_string(),
                    locations: vec![],
                    path: vec![],
                }])
            })?;

            let token = result.customer_access_token.ok_or_else(|| {
                ShopifyError::GraphQL(vec![super::GraphQLError {
                    message: "No access token returned from password reset".to_string(),
                    locations: vec![],
                    path: vec![],
                }])
            })?;

            return Ok((
                StorefrontCustomer {
                    id: customer.id,
                    email: customer.email,
                    first_name: None,
                    last_name: None,
                },
                StorefrontAccessToken {
                    access_token: token.access_token,
                    expires_at: token.expires_at,
                },
            ));
        }

        Err(ShopifyError::GraphQL(vec![super::GraphQLError {
            message: "Failed to reset password".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Renew a customer access token before it expires.
    ///
    /// # Arguments
    ///
    /// * `access_token` - The current access token
    ///
    /// # Errors
    ///
    /// Returns an error if the token is invalid or expired.
    #[instrument(skip(self, access_token))]
    pub async fn renew_access_token(
        &self,
        access_token: &str,
    ) -> Result<StorefrontAccessToken, ShopifyError> {
        let variables = customer_access_token_renew::Variables {
            customer_access_token: access_token.to_string(),
        };

        let data = self.execute::<CustomerAccessTokenRenew>(variables).await?;

        if let Some(result) = data.customer_access_token_renew {
            // Check for user errors
            if !result.user_errors.is_empty() {
                let errors: Vec<_> = result
                    .user_errors
                    .iter()
                    .map(|e| e.message.as_str())
                    .collect();
                return Err(ShopifyError::UserError(errors.join("; ")));
            }

            if let Some(token) = result.customer_access_token {
                return Ok(StorefrontAccessToken {
                    access_token: token.access_token,
                    expires_at: token.expires_at,
                });
            }
        }

        Err(ShopifyError::GraphQL(vec![super::GraphQLError {
            message: "Failed to renew access token".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Delete a customer access token (logout).
    ///
    /// # Arguments
    ///
    /// * `access_token` - The access token to delete
    ///
    /// # Errors
    ///
    /// Returns an error if the token is invalid.
    #[instrument(skip(self, access_token))]
    pub async fn delete_access_token(&self, access_token: &str) -> Result<(), ShopifyError> {
        let variables = customer_access_token_delete::Variables {
            customer_access_token: access_token.to_string(),
        };

        let data = self.execute::<CustomerAccessTokenDelete>(variables).await?;

        if let Some(result) = data.customer_access_token_delete {
            // Check for user errors
            if !result.user_errors.is_empty() {
                let errors: Vec<_> = result
                    .user_errors
                    .iter()
                    .map(|e| e.message.as_str())
                    .collect();
                return Err(ShopifyError::UserError(errors.join("; ")));
            }
        }

        Ok(())
    }

    /// Get customer information using an access token.
    ///
    /// # Arguments
    ///
    /// * `access_token` - The customer's access token
    ///
    /// # Errors
    ///
    /// Returns an error if the token is invalid or expired.
    #[instrument(skip(self, access_token))]
    pub async fn get_customer_by_token(
        &self,
        access_token: &str,
    ) -> Result<StorefrontCustomer, ShopifyError> {
        // For customer-scoped queries, we need to include the access token in the request
        // This requires a modified execute method that accepts an access token header
        let request_body = GetCustomerByToken::build_query(get_customer_by_token::Variables {});

        let response = self
            .inner
            .client
            .post(&self.inner.endpoint)
            .header("Shopify-Storefront-Private-Token", &self.inner.access_token)
            .header("X-Shopify-Customer-Access-Token", access_token)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            return Err(ShopifyError::GraphQL(vec![super::GraphQLError {
                message: format!("HTTP {status}"),
                locations: vec![],
                path: vec![],
            }]));
        }

        let response: graphql_client::Response<get_customer_by_token::ResponseData> =
            serde_json::from_str(&response_text)?;

        if let Some(data) = response.data
            && let Some(customer) = data.customer
        {
            return Ok(StorefrontCustomer {
                id: customer.id,
                email: customer.email,
                first_name: customer.first_name,
                last_name: customer.last_name,
            });
        }

        Err(ShopifyError::GraphQL(vec![super::GraphQLError {
            message: "Customer not found or token invalid".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }
}

// =============================================================================
// Customer Types
// =============================================================================

/// A customer from the Storefront API.
///
/// This is a simplified customer type for authentication purposes.
/// For full customer data, use the Customer Account API.
#[derive(Debug, Clone)]
pub struct StorefrontCustomer {
    /// Shopify customer ID (e.g., `gid://shopify/Customer/123`)
    pub id: String,
    /// Customer's email address
    pub email: Option<String>,
    /// Customer's first name
    pub first_name: Option<String>,
    /// Customer's last name
    pub last_name: Option<String>,
}

/// An access token from the Storefront API customer authentication.
///
/// This token is used to authenticate customer-scoped requests.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StorefrontAccessToken {
    /// The access token string
    pub access_token: String,
    /// When the token expires (ISO 8601 format)
    pub expires_at: String,
}

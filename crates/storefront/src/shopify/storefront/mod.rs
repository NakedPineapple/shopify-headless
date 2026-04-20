//! Shopify Storefront API client implementation.
//!
//! Uses `graphql_client` for type-safe queries with `reqwest` 0.13 for HTTP.
//! Caches products and collections using `moka` (5-minute TTL).

mod cache;
mod conversions;
pub mod queries;
pub mod schema_validation;

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
use tracing::{debug, instrument, warn};

use crate::config::ShopifyStorefrontConfig;
use crate::shopify::ShopifyError;
use crate::shopify::types::{
    ActivePromotions, Cart, CartLineInput, CartLineUpdateInput, CartRecommendations, CartUserError,
    Collection, CollectionConnection, Product, ProductConnection, ProductRecommendationIntent,
};

use cache::CacheValue;
use conversions::{
    convert_add_user_error, convert_cart, convert_collection, convert_collection_connection,
    convert_discount_user_error, convert_note_user_error, convert_product, convert_product_by_id,
    convert_product_connection, convert_product_recommendation, convert_remove_user_error,
    convert_update_user_error, convert_user_error,
};
use queries::{
    AddToCart, CreateCart, CustomerCreate, GetCart, GetCollectionByHandle, GetCollections,
    GetProductByHandle, GetProductRecommendations, GetProducts, GetShopMetafield, RemoveFromCart,
    UpdateCartDiscountCodes, UpdateCartLines, UpdateCartNote, add_to_cart, create_cart,
    customer_create, get_cart, get_collection_by_handle, get_collections, get_product_by_handle,
    get_product_recommendations, get_products, get_shop_metafield, remove_from_cart,
    update_cart_discount_codes, update_cart_lines, update_cart_note,
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
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be built (invalid TLS backend).
    #[must_use]
    pub fn new(config: &ShopifyStorefrontConfig) -> Self {
        let cache = Cache::builder()
            .max_capacity(1000)
            .time_to_live(Duration::from_mins(5))
            .build();

        let endpoint = format!(
            "https://{}/api/{}/graphql.json",
            config.store, config.api_version
        );

        let client = reqwest::Client::builder()
            .user_agent("NakedPineapple/1.0")
            .build()
            .expect("reqwest client with default user-agent builds successfully");

        Self {
            inner: Arc::new(StorefrontClientInner {
                client,
                endpoint,
                access_token: config.storefront_private_token.expose_secret().to_string(),
                cache,
            }),
        }
    }

    /// Convert `graphql_client` errors to our error type.
    fn convert_graphql_errors(errors: Vec<graphql_client::Error>) -> Vec<super::GraphQLError> {
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
                            graphql_client::PathFragment::Key(s) => serde_json::Value::String(s),
                            graphql_client::PathFragment::Index(i) => {
                                serde_json::Value::Number(i.into())
                            }
                        })
                        .collect()
                }),
            })
            .collect()
    }

    /// Execute a GraphQL query.
    #[instrument(skip(self, variables))]
    async fn execute<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<Q::ResponseData, ShopifyError>
    where
        Q::Variables: serde::Serialize,
    {
        let query_name = std::any::type_name::<Q>()
            .split("::")
            .last()
            .unwrap_or("Unknown");
        debug!(query = %query_name, "Executing Shopify Storefront GraphQL query");

        let start = std::time::Instant::now();
        let request_body = Q::build_query(variables);

        let response = self
            .inner
            .client
            .post(&self.inner.endpoint)
            .header("Shopify-Storefront-Private-Token", &self.inner.access_token)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(1);
            warn!(
                query = %query_name,
                retry_after_secs = %retry_after,
                duration_ms = %start.elapsed().as_millis(),
                "Rate limited by Shopify Storefront API"
            );
            return Err(ShopifyError::RateLimited(retry_after));
        }

        let response_text = response.text().await?;
        if !status.is_success() {
            let body_preview: String = response_text.chars().take(500).collect();
            warn!(
                query = %query_name,
                status = %status,
                body = %body_preview,
                duration_ms = %start.elapsed().as_millis(),
                "Shopify Storefront API returned non-success status"
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

        let response: Response<Q::ResponseData> = match serde_json::from_str(&response_text) {
            Ok(r) => r,
            Err(e) => {
                let body_preview: String = response_text.chars().take(500).collect();
                warn!(
                    query = %query_name,
                    error = %e,
                    body = %body_preview,
                    duration_ms = %start.elapsed().as_millis(),
                    "Failed to parse Shopify Storefront GraphQL response"
                );
                return Err(ShopifyError::Parse(e));
            }
        };

        if let Some(errors) = response.errors
            && !errors.is_empty()
        {
            let error_messages: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
            warn!(
                query = %query_name,
                errors = ?error_messages,
                duration_ms = %start.elapsed().as_millis(),
                "GraphQL errors in Storefront API response"
            );
            return Err(ShopifyError::GraphQL(Self::convert_graphql_errors(errors)));
        }

        debug!(
            query = %query_name,
            status = %status,
            duration_ms = %start.elapsed().as_millis(),
            "Shopify Storefront GraphQL query completed successfully"
        );

        response.data.ok_or_else(|| {
            let body_preview: String = response_text.chars().take(500).collect();
            warn!(
                query = %query_name,
                body = %body_preview,
                duration_ms = %start.elapsed().as_millis(),
                "Shopify Storefront GraphQL response has no data and no errors"
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

    /// Get a product's title by its ID.
    ///
    /// This is a lightweight query used for GWP auto-add display.
    ///
    /// # Errors
    ///
    /// Returns an error if the product is not found or the API request fails.
    #[instrument(skip(self), fields(id = %id))]
    pub async fn get_product_title_by_id(&self, id: &str) -> Result<String, ShopifyError> {
        use queries::{GetProductTitleById, get_product_title_by_id::Variables};

        let variables = Variables { id: id.to_string() };
        let data = self.execute::<GetProductTitleById>(variables).await?;

        data.product
            .map(|p| p.title)
            .ok_or_else(|| ShopifyError::NotFound(format!("Product not found: {id}")))
    }

    /// Get a product by its ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the product is not found or the API request fails.
    #[instrument(skip(self), fields(id = %id))]
    pub async fn get_product_by_id(&self, id: &str) -> Result<Product, ShopifyError> {
        use queries::{GetProductById, get_product_by_id::Variables};

        let cache_key = format!("product_id:{id}");

        // Check cache
        if let Some(CacheValue::Product(product)) = self.inner.cache.get(&cache_key).await {
            debug!("Cache hit for product by ID");
            return Ok(*product);
        }

        let variables = Variables {
            id: id.to_string(),
            image_count: Some(1),
            variant_count: Some(1),
        };

        let data = self.execute::<GetProductById>(variables).await?;

        let product_data = data
            .product
            .ok_or_else(|| ShopifyError::NotFound(format!("Product not found: {id}")))?;

        let product = convert_product_by_id(product_data);

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

        // Debug: Log the GraphQL variables being sent
        debug!(
            handle = %handle,
            product_count = ?product_count,
            sort_key = ?variables.sort_key,
            reverse = ?variables.reverse,
            has_filters = variables.filters.is_some(),
            filter_count = variables.filters.as_ref().map_or(0, Vec::len),
            cache_key = %cache_key,
            "Sending GraphQL request for collection"
        );

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
    // Shop Methods
    // =========================================================================

    /// Get active promotions from shop metafield.
    ///
    /// Fetches the `custom.active_promotions` metafield and parses it as JSON.
    /// Returns an empty `ActivePromotions` if the metafield doesn't exist or is invalid.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails. Invalid JSON or schema validation
    /// failures are handled gracefully by returning default empty promotions.
    #[instrument(skip(self))]
    pub async fn get_active_promotions(&self) -> Result<ActivePromotions, ShopifyError> {
        let cache_key = "shop:active_promotions".to_string();

        // Check cache
        if let Some(CacheValue::ActivePromotions(promotions)) =
            self.inner.cache.get(&cache_key).await
        {
            debug!("Cache hit for active promotions");
            return Ok(promotions);
        }

        let variables = get_shop_metafield::Variables {
            namespace: "custom".to_string(),
            key: "active_promotions".to_string(),
        };

        let data = self.execute::<GetShopMetafield>(variables).await?;

        let promotions = if let Some(m) = data.shop.metafield {
            debug!(raw_value = %m.value, "Got active_promotions metafield");

            // Validate against schema before parsing
            if let Err(e) = schema_validation::validate_str(&m.value) {
                warn!(error = %e, value = %m.value, "Active promotions metafield failed schema validation");
                ActivePromotions::default()
            } else {
                serde_json::from_str::<ActivePromotions>(&m.value)
                    .inspect_err(|e| {
                        warn!(error = %e, value = %m.value, "Failed to parse active_promotions metafield JSON");
                    })
                    .ok()
                    .unwrap_or_default()
            }
        } else {
            debug!("No active_promotions metafield found");
            ActivePromotions::default()
        };

        let filtered_promotions = filter_promotions_by_date(promotions);

        debug!(
            banners = filtered_promotions.banners.len(),
            progress_tracking = filtered_promotions.progress_tracking.len(),
            qualifying_rules = filtered_promotions.qualifying_rules.len(),
            "Loaded active promotions from metafield"
        );

        // Cache the result
        self.inner
            .cache
            .insert(
                cache_key,
                CacheValue::ActivePromotions(filtered_promotions.clone()),
            )
            .await;

        Ok(filtered_promotions)
    }

    /// Get cart recommendations from shop metafield.
    ///
    /// Fetches the `custom.cart_recommendations` metafield and parses it as JSON.
    /// Returns an empty `CartRecommendations` if the metafield doesn't exist or is invalid.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails. Invalid JSON or schema validation
    /// failures are handled gracefully by returning default empty recommendations.
    #[instrument(skip(self))]
    pub async fn get_cart_recommendations(&self) -> Result<CartRecommendations, ShopifyError> {
        let cache_key = "shop:cart_recommendations".to_string();

        // Check cache
        if let Some(CacheValue::CartRecommendations(recommendations)) =
            self.inner.cache.get(&cache_key).await
        {
            debug!("Cache hit for cart recommendations");
            return Ok(recommendations);
        }

        let variables = get_shop_metafield::Variables {
            namespace: "custom".to_string(),
            key: "cart_recommendations".to_string(),
        };

        let data = self.execute::<GetShopMetafield>(variables).await?;

        let recommendations = if let Some(m) = data.shop.metafield {
            debug!(raw_value = %m.value, "Got cart_recommendations metafield");

            // Validate against schema before parsing
            if let Err(e) = schema_validation::validate_cart_recommendations_str(&m.value) {
                warn!(error = %e, value = %m.value, "Cart recommendations metafield failed schema validation");
                CartRecommendations::default()
            } else {
                serde_json::from_str::<CartRecommendations>(&m.value)
                    .inspect_err(|e| {
                        warn!(error = %e, value = %m.value, "Failed to parse cart_recommendations metafield JSON");
                    })
                    .ok()
                    .unwrap_or_default()
            }
        } else {
            debug!("No cart_recommendations metafield found");
            CartRecommendations::default()
        };

        debug!(
            product_relations = recommendations.product_relations.len(),
            "Loaded cart recommendations from metafield"
        );

        // Cache the result
        self.inner
            .cache
            .insert(
                cache_key,
                CacheValue::CartRecommendations(recommendations.clone()),
            )
            .await;

        Ok(recommendations)
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

// =============================================================================
// Helper Functions
// =============================================================================

/// Filter promotions to only include those that are currently active.
///
/// Filtering is based on `qualifying_rules.starts_at/ends_at`. Banners and
/// `progress_tracking` entries are filtered to only those with an active rule.
fn filter_promotions_by_date(promotions: ActivePromotions) -> ActivePromotions {
    let now = chrono::Utc::now();

    debug!(
        banners_before_filter = promotions.banners.len(),
        progress_tracking_before_filter = promotions.progress_tracking.len(),
        rules_before_filter = promotions.qualifying_rules.len(),
        "Filtering promotions by date"
    );

    // First filter qualifying rules by date
    let active_rules: Vec<_> = promotions
        .qualifying_rules
        .into_iter()
        .filter(|r| {
            is_promotion_active(
                r.starts_at.as_ref(),
                r.ends_at.as_ref(),
                &now,
                "Rule",
                &r.id,
            )
        })
        .collect();

    // Build a set of active rule IDs for efficient lookup
    let active_rule_ids: std::collections::HashSet<&str> =
        active_rules.iter().map(|r| r.id.as_str()).collect();

    // Filter banners to only those with an active qualifying rule
    let banners = promotions
        .banners
        .into_iter()
        .filter(|b| {
            let active = active_rule_ids.contains(b.id.as_str());
            if !active {
                debug!(
                    id = %b.id,
                    "Banner filtered out - no active qualifying rule"
                );
            }
            active
        })
        .collect();

    // Filter progress tracking to only those with an active qualifying rule
    let progress_tracking = promotions
        .progress_tracking
        .into_iter()
        .filter(|p| {
            let active = active_rule_ids.contains(p.id.as_str());
            if !active {
                debug!(
                    id = %p.id,
                    "Progress tracking filtered out - no active qualifying rule"
                );
            }
            active
        })
        .collect();

    ActivePromotions {
        banners,
        progress_tracking,
        qualifying_rules: active_rules,
    }
}

/// Check if a promotion is currently active based on its start and end dates.
fn is_promotion_active(
    starts_at: Option<&String>,
    ends_at: Option<&String>,
    now: &chrono::DateTime<chrono::Utc>,
    kind: &str,
    id: &str,
) -> bool {
    let started = starts_at
        .is_none_or(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok_and(|dt| dt <= *now));
    let not_expired =
        ends_at.is_none_or(|e| chrono::DateTime::parse_from_rfc3339(e).is_ok_and(|dt| dt > *now));
    let active = started && not_expired;

    if !active {
        debug!(
            kind = kind,
            id = id,
            starts_at = ?starts_at,
            ends_at = ?ends_at,
            started = started,
            not_expired = not_expired,
            "{kind} filtered out by date"
        );
    }

    active
}

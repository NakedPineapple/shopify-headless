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
        AdminProduct, AdminProductConnection, AdminProductVariant, Collection,
        CollectionConnection, CollectionProduct, CollectionWithProducts, Customer,
        CustomerConnection, DiscountCode, DiscountCodeConnection, DiscountStatus, DiscountValue,
        FulfillmentOrder, FulfillmentOrderLineItem, GiftCard, GiftCardConnection, Image,
        InventoryLevel, InventoryLevelConnection, Location, LocationConnection, Money, Order,
        OrderConnection, PageInfo, Payout, PayoutConnection, PayoutStatus, StagedUploadTarget,
    },
};

mod conversions;
pub mod queries;

use conversions::{
    convert_customer, convert_customer_connection, convert_inventory_level_connection,
    convert_location_connection, convert_order, convert_order_connection, convert_product,
    convert_product_connection,
};
use queries::{
    CollectionAddProductsV2, CollectionCreate, CollectionDelete, CollectionRemoveProducts,
    CollectionUpdate, DiscountCodeBasicCreate, DiscountCodeBasicUpdate, DiscountCodeDeactivate,
    FileDelete, FileUpdate, GetCollection, GetCollectionWithProducts, GetCollections,
    GetCurrentPublication, GetCustomer, GetCustomers, GetDiscountCode, GetDiscountCodes,
    GetGiftCards, GetInventoryLevels, GetLocations, GetOrder, GetOrders, GetPayout, GetPayouts,
    GetProduct, GetProducts, GiftCardCreate, GiftCardUpdate, InventoryAdjustQuantities,
    InventorySetQuantities, OrderCancel, OrderMarkAsPaid, OrderUpdate, ProductCreate,
    ProductDelete, ProductReorderMedia, ProductSetMedia, ProductUpdate, ProductVariantsBulkUpdate,
    PublishablePublish, PublishableUnpublish, StagedUploadsCreate,
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

    /// Create a new product.
    ///
    /// # Arguments
    ///
    /// * `title` - Product title
    /// * `description_html` - HTML description
    /// * `vendor` - Vendor name
    /// * `product_type` - Product type/category
    /// * `tags` - Product tags
    /// * `status` - Product status (ACTIVE, DRAFT, ARCHIVED)
    ///
    /// # Returns
    ///
    /// Returns the created product's ID on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn create_product(
        &self,
        title: &str,
        description_html: Option<&str>,
        vendor: Option<&str>,
        product_type: Option<&str>,
        tags: Vec<String>,
        status: &str,
    ) -> Result<String, AdminShopifyError> {
        use queries::product_create::{ProductInput, ProductStatus, Variables};

        let status_enum = match status.to_uppercase().as_str() {
            "ACTIVE" => ProductStatus::ACTIVE,
            "ARCHIVED" => ProductStatus::ARCHIVED,
            _ => ProductStatus::DRAFT,
        };

        let variables = Variables {
            input: ProductInput {
                title: Some(title.to_string()),
                description_html: description_html.map(String::from),
                vendor: vendor.map(String::from),
                product_type: product_type.map(String::from),
                tags: Some(tags),
                status: Some(status_enum),
                // Other optional fields
                handle: None,
                seo: None,
                category: None,
                gift_card: None,
                gift_card_template_suffix: None,
                requires_selling_plan: None,
                template_suffix: None,
                collections_to_join: None,
                collections_to_leave: None,
                combined_listing_role: None,
                id: None,
                redirect_new_handle: None,
                claim_ownership: None,
                metafields: None,
                product_options: None,
            },
        };

        let response = self.execute::<ProductCreate>(variables).await?;

        if let Some(payload) = response.product_create {
            // Check for user errors
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(product) = payload.product {
                return Ok(product.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No product returned from create".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update an existing product.
    ///
    /// # Arguments
    ///
    /// * `id` - Product ID
    /// * `title` - Product title
    /// * `description_html` - HTML description
    /// * `vendor` - Vendor name
    /// * `product_type` - Product type/category
    /// * `tags` - Product tags
    /// * `status` - Product status (ACTIVE, DRAFT, ARCHIVED)
    ///
    /// # Returns
    ///
    /// Returns the updated product's ID on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input))]
    pub async fn update_product(
        &self,
        id: &str,
        input: ProductUpdateInput<'_>,
    ) -> Result<String, AdminShopifyError> {
        use queries::product_update::{ProductInput, ProductStatus, Variables};

        let status_enum = input.status.map(|s| match s.to_uppercase().as_str() {
            "ACTIVE" => ProductStatus::ACTIVE,
            "ARCHIVED" => ProductStatus::ARCHIVED,
            _ => ProductStatus::DRAFT,
        });

        let variables = Variables {
            input: ProductInput {
                id: Some(id.to_string()),
                title: input.title.map(String::from),
                description_html: input.description_html.map(String::from),
                vendor: input.vendor.map(String::from),
                product_type: input.product_type.map(String::from),
                tags: input.tags,
                status: status_enum,
                // Other optional fields
                handle: None,
                seo: None,
                category: None,
                gift_card: None,
                gift_card_template_suffix: None,
                requires_selling_plan: None,
                template_suffix: None,
                collections_to_join: None,
                collections_to_leave: None,
                combined_listing_role: None,
                redirect_new_handle: None,
                claim_ownership: None,
                metafields: None,
                product_options: None,
            },
        };

        let response = self.execute::<ProductUpdate>(variables).await?;

        if let Some(payload) = response.product_update {
            // Check for user errors
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(product) = payload.product {
                return Ok(product.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No product returned from update".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Delete a product.
    ///
    /// # Arguments
    ///
    /// * `id` - Product ID to delete
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn delete_product(&self, id: &str) -> Result<String, AdminShopifyError> {
        use queries::product_delete::{ProductDeleteInput, Variables};

        let variables = Variables {
            input: ProductDeleteInput { id: id.to_string() },
        };

        let response = self.execute::<ProductDelete>(variables).await?;

        if let Some(payload) = response.product_delete {
            // Check for user errors
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(deleted_id) = payload.deleted_product_id {
                return Ok(deleted_id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Product deletion failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update a product variant.
    ///
    /// # Arguments
    ///
    /// * `product_id` - Product ID the variant belongs to
    /// * `variant_id` - Variant ID to update
    /// * `price` - Optional new price
    /// * `compare_at_price` - Optional compare-at price
    /// * `sku` - Optional SKU (updated through inventory item)
    /// * `barcode` - Optional barcode
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn update_variant(
        &self,
        product_id: &str,
        variant_id: &str,
        price: Option<&str>,
        compare_at_price: Option<&str>,
        sku: Option<&str>,
        barcode: Option<&str>,
    ) -> Result<AdminProductVariant, AdminShopifyError> {
        use queries::product_variants_bulk_update::{
            InventoryItemInput, ProductVariantsBulkInput, Variables,
        };

        // Build inventory item input if SKU is being updated
        let inventory_item = sku.map(|s| InventoryItemInput {
            sku: Some(s.to_string()),
            cost: None,
            tracked: None,
            country_code_of_origin: None,
            harmonized_system_code: None,
            country_harmonized_system_codes: None,
            province_code_of_origin: None,
            measurement: None,
            requires_shipping: None,
        });

        let variables = Variables {
            product_id: product_id.to_string(),
            variants: vec![ProductVariantsBulkInput {
                id: Some(variant_id.to_string()),
                price: price.map(String::from),
                compare_at_price: compare_at_price.map(String::from),
                barcode: barcode.map(String::from),
                inventory_item,
                inventory_policy: None,
                inventory_quantities: None,
                quantity_adjustments: None,
                media_src: None,
                media_id: None,
                metafields: None,
                option_values: None,
                requires_components: None,
                tax_code: None,
                taxable: None,
                unit_price_measurement: None,
                show_unit_price: None,
            }],
        };

        let response = self.execute::<ProductVariantsBulkUpdate>(variables).await?;

        if let Some(payload) = response.product_variants_bulk_update {
            // Check for user errors
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            // Return the updated variant
            if let Some(variant) = payload.product_variants.and_then(|v| v.into_iter().next()) {
                return Ok(AdminProductVariant {
                    id: variant.id,
                    title: variant.title,
                    sku: variant.sku,
                    barcode: variant.barcode,
                    price: Money {
                        amount: variant.price,
                        currency_code: "USD".to_string(), // Default currency
                    },
                    compare_at_price: variant.compare_at_price.map(|p| Money {
                        amount: p,
                        currency_code: "USD".to_string(),
                    }),
                    inventory_quantity: variant.inventory_quantity.unwrap_or(0),
                    inventory_item_id: String::new(), // Not returned in this mutation
                    inventory_management: None,
                    weight: None,
                    weight_unit: None,
                    requires_shipping: true, // Default
                    image: None,
                    created_at: None,
                    updated_at: None,
                });
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Variant update failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    // =========================================================================
    // Media methods
    // =========================================================================

    /// Delete files (images, videos, etc.) from the store.
    ///
    /// # Arguments
    ///
    /// * `file_ids` - List of file IDs to delete
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn delete_files(
        &self,
        file_ids: Vec<String>,
    ) -> Result<Vec<String>, AdminShopifyError> {
        use queries::file_delete::Variables;

        let variables = Variables { file_ids };

        let response = self.execute::<FileDelete>(variables).await?;

        if let Some(payload) = response.file_delete {
            // Check for user errors
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(payload.deleted_file_ids.unwrap_or_default());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "File deletion failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Reorder product media (images).
    ///
    /// # Arguments
    ///
    /// * `product_id` - The product ID
    /// * `moves` - List of media moves (id, `new_position`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn reorder_product_media(
        &self,
        product_id: &str,
        moves: Vec<(String, i64)>,
    ) -> Result<(), AdminShopifyError> {
        use queries::product_reorder_media::{MoveInput, Variables};

        let move_inputs: Vec<MoveInput> = moves
            .into_iter()
            .map(|(id, new_position)| MoveInput {
                id,
                new_position: new_position.to_string(),
            })
            .collect();

        let variables = Variables {
            id: product_id.to_string(),
            moves: move_inputs,
        };

        let response = self.execute::<ProductReorderMedia>(variables).await?;

        if let Some(payload) = response.product_reorder_media {
            // Check for media user errors
            if !payload.media_user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .media_user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Media reorder failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update media alt text.
    ///
    /// # Arguments
    ///
    /// * `media_id` - The media/file ID to update
    /// * `alt_text` - The new alt text
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn update_media_alt_text(
        &self,
        media_id: &str,
        alt_text: &str,
    ) -> Result<(), AdminShopifyError> {
        use queries::file_update::{FileUpdateInput, Variables};

        let variables = Variables {
            files: vec![FileUpdateInput {
                id: media_id.to_string(),
                alt: Some(alt_text.to_string()),
                original_source: None,
                preview_image_source: None,
                filename: None,
                references_to_add: None,
                references_to_remove: None,
            }],
        };

        let response = self.execute::<FileUpdate>(variables).await?;

        if let Some(payload) = response.file_update {
            // Check for user errors
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Media alt text update failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Create a staged upload target for uploading files.
    ///
    /// # Arguments
    ///
    /// * `filename` - The filename to upload
    /// * `mime_type` - The MIME type (e.g., "image/jpeg")
    /// * `file_size` - The file size in bytes
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn create_staged_upload(
        &self,
        filename: &str,
        mime_type: &str,
        file_size: i64,
        resource: &str,
    ) -> Result<StagedUploadTarget, AdminShopifyError> {
        use queries::staged_uploads_create::{
            StagedUploadHttpMethodType, StagedUploadInput,
            StagedUploadTargetGenerateUploadResource, Variables,
        };

        let resource_type = match resource {
            "VIDEO" => StagedUploadTargetGenerateUploadResource::VIDEO,
            "FILE" => StagedUploadTargetGenerateUploadResource::FILE,
            _ => StagedUploadTargetGenerateUploadResource::IMAGE,
        };

        let variables = Variables {
            input: vec![StagedUploadInput {
                filename: filename.to_string(),
                mime_type: mime_type.to_string(),
                resource: resource_type,
                file_size: Some(file_size.to_string()),
                http_method: Some(StagedUploadHttpMethodType::POST),
            }],
        };

        let response = self.execute::<StagedUploadsCreate>(variables).await?;

        if let Some(payload) = response.staged_uploads_create {
            // Check for user errors
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(targets) = payload.staged_targets
                && let Some(target) = targets.into_iter().next()
            {
                let parameters: Vec<(String, String)> = target
                    .parameters
                    .into_iter()
                    .map(|p| (p.name, p.value))
                    .collect();

                return Ok(StagedUploadTarget {
                    url: target.url.unwrap_or_default(),
                    resource_url: target.resource_url.unwrap_or_default(),
                    parameters,
                });
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Staged upload creation failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Attach uploaded media to a product.
    ///
    /// Uses `productSet` mutation with files parameter (non-deprecated).
    ///
    /// # Arguments
    ///
    /// * `product_id` - The product ID
    /// * `resource_url` - The URL returned from staged upload
    /// * `alt_text` - Optional alt text for the image
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn attach_media_to_product(
        &self,
        product_id: &str,
        resource_url: &str,
        alt_text: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        use queries::product_set_media::{
            FileSetInput, ProductSetIdentifiers, ProductSetInput, Variables,
        };

        let variables = Variables {
            input: ProductSetInput {
                files: Some(vec![FileSetInput {
                    id: None,
                    original_source: Some(resource_url.to_string()),
                    alt: alt_text.map(String::from),
                    content_type: None,
                    filename: None,
                    duplicate_resolution_mode: None,
                }]),
                // All other fields are optional, set to None
                description_html: None,
                handle: None,
                seo: None,
                product_type: None,
                category: None,
                tags: None,
                template_suffix: None,
                gift_card_template_suffix: None,
                title: None,
                vendor: None,
                gift_card: None,
                redirect_new_handle: None,
                collections: None,
                metafields: None,
                variants: None,
                status: None,
                requires_selling_plan: None,
                product_options: None,
                claim_ownership: None,
                combined_listing_role: None,
            },
            identifier: Some(ProductSetIdentifiers {
                id: Some(product_id.to_string()),
                handle: None,
                custom_id: None,
            }),
        };

        let response = self.execute::<ProductSetMedia>(variables).await?;

        if let Some(payload) = response.product_set {
            // Check for user errors
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            // Media was attached successfully (it may still be processing)
            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Media attachment failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    // =========================================================================
    // Collection methods
    // =========================================================================

    /// Get a collection by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self), fields(collection_id = %id))]
    pub async fn get_collection(&self, id: &str) -> Result<Option<Collection>, AdminShopifyError> {
        let variables = queries::get_collection::Variables { id: id.to_string() };

        let response = self.execute::<GetCollection>(variables).await?;

        Ok(response.collection.map(|c| {
            use super::types::{CollectionRule, CollectionRuleSet, CollectionSeo};

            Collection {
                id: c.id,
                title: c.title,
                handle: c.handle,
                description: c.description,
                description_html: Some(c.description_html),
                products_count: c.products_count.map_or(0, |pc| pc.count),
                image: c.image.map(|img| Image {
                    id: img.id,
                    url: img.url,
                    alt_text: img.alt_text,
                    width: None,
                    height: None,
                }),
                updated_at: Some(c.updated_at),
                rule_set: c.rule_set.map(|rs| CollectionRuleSet {
                    applied_disjunctively: rs.applied_disjunctively,
                    rules: rs
                        .rules
                        .into_iter()
                        .map(|r| CollectionRule {
                            column: format!("{:?}", r.column),
                            relation: format!("{:?}", r.relation),
                            condition: r.condition,
                        })
                        .collect(),
                }),
                sort_order: Some(format!("{:?}", c.sort_order)),
                seo: Some(CollectionSeo {
                    title: c.seo.title,
                    description: c.seo.description,
                }),
                published_on_current_publication: Some(c.published_on_current_publication),
            }
        }))
    }

    /// Get a paginated list of collections.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_collections(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
    ) -> Result<CollectionConnection, AdminShopifyError> {
        let variables = queries::get_collections::Variables {
            first: Some(first),
            after,
            query,
        };

        let response = self.execute::<GetCollections>(variables).await?;

        let collections: Vec<Collection> = response
            .collections
            .edges
            .into_iter()
            .map(|e| {
                let c = e.node;
                Collection {
                    id: c.id,
                    title: c.title,
                    handle: c.handle,
                    description: c.description,
                    description_html: None,
                    products_count: c.products_count.map_or(0, |pc| pc.count),
                    image: c.image.map(|img| Image {
                        id: img.id,
                        url: img.url,
                        alt_text: img.alt_text,
                        width: None,
                        height: None,
                    }),
                    updated_at: Some(c.updated_at),
                    // List view doesn't include these extended fields
                    rule_set: None,
                    sort_order: None,
                    seo: None,
                    published_on_current_publication: None,
                }
            })
            .collect();

        Ok(CollectionConnection {
            collections,
            page_info: PageInfo {
                has_next_page: response.collections.page_info.has_next_page,
                has_previous_page: false,
                start_cursor: None,
                end_cursor: response.collections.page_info.end_cursor,
            },
        })
    }

    /// Create a new collection.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn create_collection(
        &self,
        title: &str,
        description_html: Option<&str>,
    ) -> Result<String, AdminShopifyError> {
        use queries::collection_create::{CollectionInput, Variables};

        let variables = Variables {
            input: CollectionInput {
                title: Some(title.to_string()),
                description_html: description_html.map(String::from),
                handle: None,
                id: None,
                image: None,
                metafields: None,
                products: None,
                redirect_new_handle: None,
                rule_set: None,
                seo: None,
                sort_order: None,
                template_suffix: None,
            },
        };

        let response = self.execute::<CollectionCreate>(variables).await?;

        if let Some(payload) = response.collection_create {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(collection) = payload.collection {
                return Ok(collection.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No collection returned from create".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update an existing collection.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn update_collection(
        &self,
        id: &str,
        title: Option<&str>,
        description_html: Option<&str>,
        sort_order: Option<&str>,
        seo_title: Option<&str>,
        seo_description: Option<&str>,
    ) -> Result<String, AdminShopifyError> {
        use queries::collection_update::{CollectionInput, SEOInput, Variables};

        // Build SEO input if any SEO field is provided
        let seo = if seo_title.is_some() || seo_description.is_some() {
            Some(SEOInput {
                title: seo_title.map(String::from),
                description: seo_description.map(String::from),
            })
        } else {
            None
        };

        // Convert sort order string to enum
        let sort_order_enum = sort_order.and_then(|s| match s {
            "BEST_SELLING" => Some(queries::collection_update::CollectionSortOrder::BEST_SELLING),
            "ALPHA_ASC" => Some(queries::collection_update::CollectionSortOrder::ALPHA_ASC),
            "ALPHA_DESC" => Some(queries::collection_update::CollectionSortOrder::ALPHA_DESC),
            "PRICE_ASC" => Some(queries::collection_update::CollectionSortOrder::PRICE_ASC),
            "PRICE_DESC" => Some(queries::collection_update::CollectionSortOrder::PRICE_DESC),
            "CREATED_DESC" => Some(queries::collection_update::CollectionSortOrder::CREATED_DESC),
            "CREATED" => Some(queries::collection_update::CollectionSortOrder::CREATED),
            "MANUAL" => Some(queries::collection_update::CollectionSortOrder::MANUAL),
            _ => None,
        });

        let variables = Variables {
            input: CollectionInput {
                id: Some(id.to_string()),
                title: title.map(String::from),
                description_html: description_html.map(String::from),
                handle: None,
                image: None,
                metafields: None,
                products: None,
                redirect_new_handle: None,
                rule_set: None,
                seo,
                sort_order: sort_order_enum,
                template_suffix: None,
            },
        };

        let response = self.execute::<CollectionUpdate>(variables).await?;

        if let Some(payload) = response.collection_update {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(collection) = payload.collection {
                return Ok(collection.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No collection returned from update".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Delete a collection.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn delete_collection(&self, id: &str) -> Result<String, AdminShopifyError> {
        use queries::collection_delete::{CollectionDeleteInput, Variables};

        let variables = Variables {
            input: CollectionDeleteInput { id: id.to_string() },
        };

        let response = self.execute::<CollectionDelete>(variables).await?;

        if let Some(payload) = response.collection_delete {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(deleted_id) = payload.deleted_collection_id {
                return Ok(deleted_id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Collection deletion failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Get a collection with its products.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self), fields(collection_id = %id))]
    pub async fn get_collection_with_products(
        &self,
        id: &str,
        first: i64,
        after: Option<String>,
    ) -> Result<Option<CollectionWithProducts>, AdminShopifyError> {
        use super::types::{CollectionRule, CollectionRuleSet, CollectionSeo};

        let variables = queries::get_collection_with_products::Variables {
            id: id.to_string(),
            first: Some(first),
            after,
        };

        let response = self.execute::<GetCollectionWithProducts>(variables).await?;

        Ok(response.collection.map(|c| {
            let products: Vec<CollectionProduct> = c
                .products
                .edges
                .into_iter()
                .map(|e| {
                    let p = e.node;
                    let min_price = &p.price_range_v2.min_variant_price;
                    let price = min_price.amount.clone();
                    let currency_code = format!("{:?}", min_price.currency_code);

                    #[allow(deprecated)]
                    CollectionProduct {
                        id: p.id,
                        title: p.title,
                        handle: p.handle,
                        status: format!("{:?}", p.status),
                        image_url: p.featured_image.map(|img| img.url),
                        total_inventory: p.total_inventory,
                        price,
                        currency_code,
                    }
                })
                .collect();

            let has_next_page = c.products.page_info.has_next_page;
            let end_cursor = c.products.page_info.end_cursor;

            let collection = Collection {
                id: c.id,
                title: c.title,
                handle: c.handle,
                description: c.description,
                description_html: Some(c.description_html),
                products_count: c.products_count.map_or(0, |pc| pc.count),
                image: c.image.map(|img| Image {
                    id: img.id,
                    url: img.url,
                    alt_text: img.alt_text,
                    width: None,
                    height: None,
                }),
                updated_at: Some(c.updated_at),
                rule_set: c.rule_set.map(|rs| CollectionRuleSet {
                    applied_disjunctively: rs.applied_disjunctively,
                    rules: rs
                        .rules
                        .into_iter()
                        .map(|r| CollectionRule {
                            column: format!("{:?}", r.column),
                            relation: format!("{:?}", r.relation),
                            condition: r.condition,
                        })
                        .collect(),
                }),
                sort_order: Some(format!("{:?}", c.sort_order)),
                seo: Some(CollectionSeo {
                    title: c.seo.title,
                    description: c.seo.description,
                }),
                published_on_current_publication: Some(c.published_on_current_publication),
            };

            CollectionWithProducts {
                collection,
                products,
                has_next_page,
                end_cursor,
            }
        }))
    }

    /// Add products to a collection.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn add_products_to_collection(
        &self,
        collection_id: &str,
        product_ids: Vec<String>,
    ) -> Result<(), AdminShopifyError> {
        let variables = queries::collection_add_products_v2::Variables {
            id: collection_id.to_string(),
            product_ids,
        };

        let response = self.execute::<CollectionAddProductsV2>(variables).await?;

        if let Some(payload) = response.collection_add_products_v2 {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Add products to collection failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Remove products from a collection.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn remove_products_from_collection(
        &self,
        collection_id: &str,
        product_ids: Vec<String>,
    ) -> Result<(), AdminShopifyError> {
        let variables = queries::collection_remove_products::Variables {
            id: collection_id.to_string(),
            product_ids,
        };

        let response = self.execute::<CollectionRemoveProducts>(variables).await?;

        if let Some(payload) = response.collection_remove_products {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Remove products from collection failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Get the current publication ID (online store).
    async fn get_current_publication_id(&self) -> Result<Option<String>, AdminShopifyError> {
        let variables = queries::get_current_publication::Variables {};
        let response = self.execute::<GetCurrentPublication>(variables).await?;

        let id = response.current_app_installation.publication.map(|p| p.id);

        Ok(id)
    }

    /// Publish a collection to the current publication.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn publish_collection(&self, collection_id: &str) -> Result<(), AdminShopifyError> {
        let publication_id = self
            .get_current_publication_id()
            .await?
            .ok_or_else(|| AdminShopifyError::NotFound("No publication found".to_string()))?;

        let variables = queries::publishable_publish::Variables {
            id: collection_id.to_string(),
            input: vec![queries::publishable_publish::PublicationInput {
                publication_id: Some(publication_id),
                publish_date: None,
            }],
        };

        let response = self.execute::<PublishablePublish>(variables).await?;

        if let Some(payload) = response.publishable_publish
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Unpublish a collection from the current publication.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn unpublish_collection(&self, collection_id: &str) -> Result<(), AdminShopifyError> {
        let publication_id = self
            .get_current_publication_id()
            .await?
            .ok_or_else(|| AdminShopifyError::NotFound("No publication found".to_string()))?;

        let variables = queries::publishable_unpublish::Variables {
            id: collection_id.to_string(),
            input: vec![queries::publishable_unpublish::PublicationInput {
                publication_id: Some(publication_id),
                publish_date: None,
            }],
        };

        let response = self.execute::<PublishableUnpublish>(variables).await?;

        if let Some(payload) = response.publishable_unpublish
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
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

    /// Update an order's note.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    /// * `note` - New note content
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn update_order_note(
        &self,
        id: &str,
        note: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        use queries::order_update::{OrderInput, Variables};

        let variables = Variables {
            input: OrderInput {
                id: id.to_string(),
                note: note.map(String::from),
                tags: None,
                custom_attributes: None,
                email: None,
                localized_fields: None,
                metafields: None,
                phone: None,
                po_number: None,
                shipping_address: None,
            },
        };

        let response = self.execute::<OrderUpdate>(variables).await?;

        if let Some(payload) = response.order_update
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Update an order's tags.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    /// * `tags` - New tags (replaces existing tags)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn update_order_tags(
        &self,
        id: &str,
        tags: Vec<String>,
    ) -> Result<(), AdminShopifyError> {
        use queries::order_update::{OrderInput, Variables};

        let variables = Variables {
            input: OrderInput {
                id: id.to_string(),
                note: None,
                tags: Some(tags),
                custom_attributes: None,
                email: None,
                localized_fields: None,
                metafields: None,
                phone: None,
                po_number: None,
                shipping_address: None,
            },
        };

        let response = self.execute::<OrderUpdate>(variables).await?;

        if let Some(payload) = response.order_update
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Mark an order as paid.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn mark_order_as_paid(&self, id: &str) -> Result<(), AdminShopifyError> {
        use queries::order_mark_as_paid::{OrderMarkAsPaidInput, Variables};

        let variables = Variables {
            input: OrderMarkAsPaidInput { id: id.to_string() },
        };

        let response = self.execute::<OrderMarkAsPaid>(variables).await?;

        if let Some(payload) = response.order_mark_as_paid
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Cancel an order.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    /// * `reason` - Cancellation reason
    /// * `notify_customer` - Whether to notify the customer
    /// * `refund` - Whether to refund the order
    /// * `restock` - Whether to restock inventory
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn cancel_order(
        &self,
        id: &str,
        reason: Option<&str>,
        notify_customer: bool,
        refund: bool,
        restock: bool,
    ) -> Result<(), AdminShopifyError> {
        use queries::order_cancel::{OrderCancelReason, Variables};

        let cancel_reason = reason.map(|r| match r.to_uppercase().as_str() {
            "CUSTOMER" => OrderCancelReason::CUSTOMER,
            "FRAUD" => OrderCancelReason::FRAUD,
            "INVENTORY" => OrderCancelReason::INVENTORY,
            "DECLINED" => OrderCancelReason::DECLINED,
            _ => OrderCancelReason::OTHER,
        });

        let variables = Variables {
            order_id: id.to_string(),
            reason: cancel_reason,
            notify_customer: Some(notify_customer),
            refund: Some(refund),
            restock: Some(restock),
            staff_note: None,
        };

        let response = self.execute::<OrderCancel>(variables).await?;

        if let Some(payload) = response.order_cancel
            && !payload.order_cancel_user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .order_cancel_user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    // =========================================================================
    // Fulfillment methods
    // =========================================================================

    // NOTE: Fulfillment operations require complex GraphQL types that are
    // auto-generated by graphql_client. These methods are stubbed and will
    // need to be implemented with the correct types during build verification.

    /// Get fulfillment orders for an order.
    ///
    /// # Arguments
    ///
    /// * `order_id` - Shopify order ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn get_fulfillment_orders(
        &self,
        order_id: &str,
    ) -> Result<Vec<FulfillmentOrder>, AdminShopifyError> {
        // TODO: Implement with GetFulfillmentOrders query
        // The auto-generated types for FulfillmentOrderStatus need Display trait
        let _ = order_id;
        Ok(vec![])
    }

    /// Create a fulfillment.
    ///
    /// # Arguments
    ///
    /// * `fulfillment_order_id` - Fulfillment order ID to fulfill
    /// * `tracking_company` - Optional shipping carrier
    /// * `tracking_number` - Optional tracking number
    /// * `tracking_url` - Optional tracking URL
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(fulfillment_order_id = %fulfillment_order_id))]
    pub async fn create_fulfillment(
        &self,
        fulfillment_order_id: &str,
        tracking_company: Option<&str>,
        tracking_number: Option<&str>,
        tracking_url: Option<&str>,
    ) -> Result<String, AdminShopifyError> {
        // TODO: Implement with FulfillmentCreateV2 mutation
        // Need to map to correct GraphQL input types
        let _ = (
            fulfillment_order_id,
            tracking_company,
            tracking_number,
            tracking_url,
        );
        Err(AdminShopifyError::UserError(
            "Fulfillment creation not yet implemented".to_string(),
        ))
    }

    /// Update fulfillment tracking info.
    ///
    /// # Arguments
    ///
    /// * `fulfillment_id` - Fulfillment ID
    /// * `tracking_company` - Shipping carrier
    /// * `tracking_number` - Tracking number
    /// * `tracking_url` - Optional tracking URL
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(fulfillment_id = %fulfillment_id))]
    pub async fn update_fulfillment_tracking(
        &self,
        fulfillment_id: &str,
        tracking_company: Option<&str>,
        tracking_number: Option<&str>,
        tracking_url: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        // TODO: Implement with FulfillmentTrackingInfoUpdateV2 mutation
        let _ = (
            fulfillment_id,
            tracking_company,
            tracking_number,
            tracking_url,
        );
        Err(AdminShopifyError::UserError(
            "Tracking update not yet implemented".to_string(),
        ))
    }

    // =========================================================================
    // Refund methods
    // =========================================================================

    /// Create a refund for an order.
    ///
    /// # Arguments
    ///
    /// * `order_id` - Shopify order ID
    /// * `note` - Refund note
    /// * `notify` - Whether to notify the customer
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn create_refund(
        &self,
        order_id: &str,
        note: Option<&str>,
        notify: bool,
    ) -> Result<String, AdminShopifyError> {
        // TODO: Implement with RefundCreate mutation
        // RefundInput has many required fields that need proper mapping
        let _ = (order_id, note, notify);
        Err(AdminShopifyError::UserError(
            "Refund creation not yet implemented".to_string(),
        ))
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
    // Location methods
    // =========================================================================

    /// Get all locations.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_locations(&self) -> Result<LocationConnection, AdminShopifyError> {
        let variables = queries::get_locations::Variables { first: Some(50) };

        let response = self.execute::<GetLocations>(variables).await?;

        Ok(convert_location_connection(response.locations))
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

    // =========================================================================
    // Gift Card methods
    // =========================================================================

    /// Get a paginated list of gift cards.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of gift cards to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_gift_cards(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
    ) -> Result<GiftCardConnection, AdminShopifyError> {
        let variables = queries::get_gift_cards::Variables {
            first: Some(first),
            after,
            query,
        };

        let response = self.execute::<GetGiftCards>(variables).await?;

        let gift_cards: Vec<GiftCard> = response
            .gift_cards
            .edges
            .into_iter()
            .map(|e| {
                let gc = e.node;
                #[allow(deprecated)]
                GiftCard {
                    id: gc.id,
                    last_characters: gc.last_characters,
                    balance: Money {
                        amount: gc.balance.amount,
                        currency_code: format!("{:?}", gc.balance.currency_code),
                    },
                    initial_value: Money {
                        amount: gc.initial_value.amount,
                        currency_code: format!("{:?}", gc.initial_value.currency_code),
                    },
                    expires_on: gc.expires_on,
                    enabled: gc.enabled,
                    created_at: gc.created_at,
                    customer_email: gc.customer.as_ref().and_then(|c| c.email.clone()),
                    customer_name: gc.customer.as_ref().map(|c| c.display_name.clone()),
                    note: None,
                }
            })
            .collect();

        Ok(GiftCardConnection {
            gift_cards,
            page_info: PageInfo {
                has_next_page: response.gift_cards.page_info.has_next_page,
                has_previous_page: false,
                start_cursor: None,
                end_cursor: response.gift_cards.page_info.end_cursor,
            },
        })
    }

    /// Create a new gift card.
    ///
    /// # Arguments
    ///
    /// * `initial_value` - Initial value amount as decimal string
    /// * `customer_id` - Optional customer to associate
    /// * `expires_on` - Optional expiration date (YYYY-MM-DD)
    /// * `note` - Optional internal note
    ///
    /// # Returns
    ///
    /// Returns a tuple of (gift card ID, gift card code) on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn create_gift_card(
        &self,
        initial_value: &str,
        customer_id: Option<&str>,
        expires_on: Option<&str>,
        note: Option<&str>,
    ) -> Result<(String, String), AdminShopifyError> {
        use queries::gift_card_create::{GiftCardCreateInput, Variables};

        let variables = Variables {
            input: GiftCardCreateInput {
                initial_value: initial_value.to_string(),
                customer_id: customer_id.map(String::from),
                expires_on: expires_on.map(String::from),
                note: note.map(String::from),
                code: None,
                template_suffix: None,
                recipient_attributes: None,
            },
        };

        let response = self.execute::<GiftCardCreate>(variables).await?;

        if let Some(payload) = response.gift_card_create {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let (Some(gc), Some(code)) = (payload.gift_card, payload.gift_card_code) {
                return Ok((gc.id, code));
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No gift card returned from create".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Disable a gift card.
    ///
    /// Note: Shopify's `GiftCardUpdate` mutation uses the gift card's global ID
    /// in the mutation itself, not in the input struct.
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID to disable
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn disable_gift_card(&self, id: &str) -> Result<(), AdminShopifyError> {
        use queries::gift_card_update::{GiftCardUpdateInput, Variables};

        let variables = Variables {
            id: id.to_string(),
            input: GiftCardUpdateInput {
                note: None,
                expires_on: None,
                customer_id: None,
                template_suffix: None,
                recipient_attributes: None,
            },
        };

        let response = self.execute::<GiftCardUpdate>(variables).await?;

        if let Some(payload) = response.gift_card_update
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    // =========================================================================
    // Discount methods
    // =========================================================================

    /// Get a paginated list of discount codes.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of discounts to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    #[allow(deprecated)] // Shopify's API deprecation, still functional
    pub async fn get_discounts(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
    ) -> Result<DiscountCodeConnection, AdminShopifyError> {
        let variables = queries::get_discount_codes::Variables {
            first: Some(first),
            after,
            query,
        };

        let response = self.execute::<GetDiscountCodes>(variables).await?;

        let discount_codes: Vec<DiscountCode> = response
            .discount_nodes
            .edges
            .into_iter()
            .filter_map(|e| {
                let node = e.node;
                let cd = node.discount;

                // Extract common fields from the union type
                // Skip automatic discounts - we only handle code-based discounts
                match cd {
                    queries::get_discount_codes::GetDiscountCodesDiscountNodesEdgesNodeDiscount::DiscountCodeBasic(basic) => {
                        // Type alias for the discount value type
                        use queries::get_discount_codes::GetDiscountCodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBasicCustomerGetsValue as BasicValue;
                        let code = basic.codes.edges.first().map(|e2| e2.node.code.clone()).unwrap_or_default();
                        let value = match basic.customer_gets.value {
                            BasicValue::DiscountPercentage(p) => {
                                Some(DiscountValue::Percentage { percentage: p.percentage })
                            }
                            BasicValue::DiscountAmount(a) => {
                                Some(DiscountValue::FixedAmount {
                                    amount: a.amount.amount,
                                    currency: format!("{:?}", a.amount.currency_code),
                                })
                            }
                            // Quantity-based discounts not yet mapped
                            BasicValue::DiscountOnQuantity => None,
                        };

                        Some(DiscountCode {
                            id: node.id,
                            title: basic.title,
                            code,
                            status: convert_discount_status(&basic.status),
                            starts_at: Some(basic.starts_at),
                            ends_at: basic.ends_at,
                            usage_limit: basic.usage_limit,
                            usage_count: basic.async_usage_count,
                            value,
                        })
                    }
                    queries::get_discount_codes::GetDiscountCodesDiscountNodesEdgesNodeDiscount::DiscountCodeBxgy(bxgy) => {
                        let code = bxgy.codes.edges.first().map(|e2| e2.node.code.clone()).unwrap_or_default();
                        Some(DiscountCode {
                            id: node.id,
                            title: bxgy.title,
                            code,
                            status: convert_discount_status(&bxgy.status),
                            starts_at: Some(bxgy.starts_at),
                            ends_at: bxgy.ends_at,
                            usage_limit: bxgy.usage_limit,
                            usage_count: bxgy.async_usage_count,
                            value: None,
                        })
                    }
                    queries::get_discount_codes::GetDiscountCodesDiscountNodesEdgesNodeDiscount::DiscountCodeFreeShipping(fs) => {
                        let code = fs.codes.edges.first().map(|e2| e2.node.code.clone()).unwrap_or_default();
                        Some(DiscountCode {
                            id: node.id,
                            title: fs.title,
                            code,
                            status: convert_discount_status(&fs.status),
                            starts_at: Some(fs.starts_at),
                            ends_at: fs.ends_at,
                            usage_limit: fs.usage_limit,
                            usage_count: fs.async_usage_count,
                            value: None,
                        })
                    }
                    // Skip app-based and automatic discounts (not managed through the Admin UI)
                    _ => None,
                }
            })
            .collect();

        Ok(DiscountCodeConnection {
            discount_codes,
            page_info: PageInfo {
                has_next_page: response.discount_nodes.page_info.has_next_page,
                has_previous_page: false,
                start_cursor: None,
                end_cursor: response.discount_nodes.page_info.end_cursor,
            },
        })
    }

    /// Create a basic discount code (percentage or fixed amount).
    ///
    /// # Arguments
    ///
    /// * `input` - Discount creation parameters
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input))]
    pub async fn create_discount(
        &self,
        input: DiscountCreateInput<'_>,
    ) -> Result<String, AdminShopifyError> {
        use queries::discount_code_basic_create::{
            DiscountCodeBasicInput, DiscountCustomerGetsInput, DiscountCustomerGetsValueInput,
            DiscountItemsInput, Variables,
        };

        // Build the value input based on percentage or amount
        let value = if let Some(pct) = input.percentage {
            DiscountCustomerGetsValueInput {
                percentage: Some(pct),
                discount_amount: None,
                discount_on_quantity: None,
            }
        } else if let Some((amt, _currency)) = input.amount {
            use queries::discount_code_basic_create::DiscountAmountInput;
            DiscountCustomerGetsValueInput {
                percentage: None,
                discount_amount: Some(DiscountAmountInput {
                    amount: Some(amt.to_string()),
                    applies_on_each_item: Some(false),
                }),
                discount_on_quantity: None,
            }
        } else {
            return Err(AdminShopifyError::UserError(
                "Must specify either percentage or amount".to_string(),
            ));
        };

        let variables = Variables {
            basic_code_discount: DiscountCodeBasicInput {
                title: Some(input.title.to_string()),
                code: Some(input.code.to_string()),
                starts_at: Some(input.starts_at.to_string()),
                ends_at: input.ends_at.map(String::from),
                usage_limit: input.usage_limit,
                customer_gets: Some(DiscountCustomerGetsInput {
                    value: Some(value),
                    items: Some(DiscountItemsInput {
                        all: Some(true),
                        collections: None,
                        products: None,
                    }),
                    applies_on_one_time_purchase: None,
                    applies_on_subscription: None,
                }),
                applies_once_per_customer: Some(false),
                combines_with: None,
                minimum_requirement: None,
                recurring_cycle_limit: None,
                context: None,
            },
        };

        let response = self.execute::<DiscountCodeBasicCreate>(variables).await?;

        if let Some(payload) = response.discount_code_basic_create {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            if let Some(node) = payload.code_discount_node {
                return Ok(node.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No discount returned from create".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Deactivate a discount code.
    ///
    /// # Arguments
    ///
    /// * `id` - Discount node ID to deactivate
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn deactivate_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        let variables = queries::discount_code_deactivate::Variables { id: id.to_string() };

        let response = self.execute::<DiscountCodeDeactivate>(variables).await?;

        if let Some(payload) = response.discount_code_deactivate
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Get a single discount code by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Discount node ID (e.g., `gid://shopify/DiscountCodeNode/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or the discount is not found.
    #[instrument(skip(self), fields(discount_id = %id))]
    pub async fn get_discount(&self, id: &str) -> Result<DiscountCode, AdminShopifyError> {
        let variables = queries::get_discount_code::Variables { id: id.to_string() };

        let response = self.execute::<GetDiscountCode>(variables).await?;

        let Some(node) = response.discount_node else {
            return Err(AdminShopifyError::NotFound(format!(
                "Discount {id} not found"
            )));
        };

        // Extract discount details from the union type
        use queries::get_discount_code::GetDiscountCodeDiscountNodeDiscount as Discount;
        match node.discount {
            Discount::DiscountCodeBasic(basic) => {
                use queries::get_discount_code::GetDiscountCodeDiscountNodeDiscountOnDiscountCodeBasicCustomerGetsValue as BasicValue;
                let code = basic
                    .codes
                    .edges
                    .first()
                    .map(|e| e.node.code.clone())
                    .unwrap_or_default();
                let value = match basic.customer_gets.value {
                    BasicValue::DiscountPercentage(p) => Some(DiscountValue::Percentage {
                        percentage: p.percentage,
                    }),
                    BasicValue::DiscountAmount(a) => Some(DiscountValue::FixedAmount {
                        amount: a.amount.amount,
                        currency: format!("{:?}", a.amount.currency_code),
                    }),
                    BasicValue::DiscountOnQuantity => None,
                };

                Ok(DiscountCode {
                    id: node.id,
                    title: basic.title,
                    code,
                    status: convert_discount_status_single(&basic.status),
                    starts_at: Some(basic.starts_at),
                    ends_at: basic.ends_at,
                    usage_limit: basic.usage_limit,
                    usage_count: basic.async_usage_count,
                    value,
                })
            }
            _ => Err(AdminShopifyError::NotFound(format!(
                "Discount {id} is not a basic discount code (BXGY, Free Shipping, and automatic discounts cannot be edited here)"
            ))),
        }
    }

    /// Update a basic discount code.
    ///
    /// # Arguments
    ///
    /// * `id` - Discount node ID
    /// * `input` - Update parameters
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input), fields(discount_id = %id))]
    pub async fn update_discount(
        &self,
        id: &str,
        input: DiscountUpdateInput<'_>,
    ) -> Result<(), AdminShopifyError> {
        use queries::discount_code_basic_update::{DiscountCodeBasicInput, Variables};

        let variables = Variables {
            id: id.to_string(),
            basic_code_discount: DiscountCodeBasicInput {
                title: input.title.map(String::from),
                code: None, // Code cannot be changed
                starts_at: input.starts_at.map(String::from),
                ends_at: input.ends_at.map(String::from),
                usage_limit: None,   // Keep existing
                customer_gets: None, // Keep existing value
                applies_once_per_customer: None,
                combines_with: None,
                minimum_requirement: None,
                recurring_cycle_limit: None,
                context: None,
            },
        };

        let response = self.execute::<DiscountCodeBasicUpdate>(variables).await?;

        if let Some(payload) = response.discount_code_basic_update
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    // =========================================================================
    // Payout methods
    // =========================================================================

    /// Get a paginated list of payouts from Shopify Payments.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of payouts to return
    /// * `after` - Cursor for pagination
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or Shopify Payments is not enabled.
    #[instrument(skip(self))]
    pub async fn get_payouts(
        &self,
        first: i64,
        after: Option<String>,
    ) -> Result<PayoutConnection, AdminShopifyError> {
        let variables = queries::get_payouts::Variables {
            first: Some(first),
            after,
        };

        let response = self.execute::<GetPayouts>(variables).await?;

        let Some(account) = response.shopify_payments_account else {
            return Err(AdminShopifyError::NotFound(
                "Shopify Payments is not enabled for this store".to_string(),
            ));
        };

        // Using deprecated `gross` field - Shopify API deprecation, still functional
        #[allow(deprecated)]
        let payouts: Vec<Payout> = account
            .payouts
            .edges
            .into_iter()
            .map(|e| {
                let p = e.node;
                Payout {
                    id: p.id,
                    legacy_resource_id: Some(p.legacy_resource_id.clone()),
                    status: convert_payout_status(&p.status),
                    net: Money {
                        amount: p.net.amount,
                        currency_code: format!("{:?}", p.net.currency_code),
                    },
                    issued_at: Some(p.issued_at),
                }
            })
            .collect();

        // balance is a Vec in the schema, take the first one if present
        let balance = account.balance.into_iter().next().map(|b| Money {
            amount: b.amount,
            currency_code: format!("{:?}", b.currency_code),
        });

        Ok(PayoutConnection {
            payouts,
            page_info: PageInfo {
                has_next_page: account.payouts.page_info.has_next_page,
                has_previous_page: false,
                start_cursor: None,
                end_cursor: account.payouts.page_info.end_cursor,
            },
            balance,
        })
    }

    /// Get a single payout by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The payout's global ID (e.g., `gid://shopify/ShopifyPaymentsPayout/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or the payout is not found.
    #[instrument(skip(self), fields(payout_id = %id))]
    pub async fn get_payout(&self, id: &str) -> Result<Payout, AdminShopifyError> {
        let variables = queries::get_payout::Variables { id: id.to_string() };

        let response = self.execute::<GetPayout>(variables).await?;

        let Some(node) = response.node else {
            return Err(AdminShopifyError::NotFound(format!(
                "Payout {id} not found"
            )));
        };

        // The node query returns a union type, we need to match on ShopifyPaymentsPayout
        use queries::get_payout::GetPayoutNode;
        match node {
            GetPayoutNode::ShopifyPaymentsPayout(p) => Ok(Payout {
                id: p.id,
                legacy_resource_id: Some(p.legacy_resource_id.clone()),
                status: convert_payout_status_single(&p.status),
                net: Money {
                    amount: p.net.amount,
                    currency_code: format!("{:?}", p.net.currency_code),
                },
                issued_at: Some(p.issued_at),
            }),
            _ => Err(AdminShopifyError::NotFound(format!(
                "Node {id} is not a payout"
            ))),
        }
    }
}

/// Convert GraphQL payout status to domain type (for single payout query).
const fn convert_payout_status_single(
    status: &queries::get_payout::ShopifyPaymentsPayoutStatus,
) -> PayoutStatus {
    match status {
        queries::get_payout::ShopifyPaymentsPayoutStatus::SCHEDULED
        | queries::get_payout::ShopifyPaymentsPayoutStatus::Other(_) => PayoutStatus::Scheduled,
        queries::get_payout::ShopifyPaymentsPayoutStatus::IN_TRANSIT => PayoutStatus::InTransit,
        queries::get_payout::ShopifyPaymentsPayoutStatus::PAID => PayoutStatus::Paid,
        queries::get_payout::ShopifyPaymentsPayoutStatus::FAILED => PayoutStatus::Failed,
        queries::get_payout::ShopifyPaymentsPayoutStatus::CANCELED => PayoutStatus::Canceled,
    }
}

/// Convert GraphQL discount status to domain type.
const fn convert_discount_status(
    status: &queries::get_discount_codes::DiscountStatus,
) -> DiscountStatus {
    match status {
        queries::get_discount_codes::DiscountStatus::ACTIVE
        | queries::get_discount_codes::DiscountStatus::Other(_) => DiscountStatus::Active,
        queries::get_discount_codes::DiscountStatus::EXPIRED => DiscountStatus::Expired,
        queries::get_discount_codes::DiscountStatus::SCHEDULED => DiscountStatus::Scheduled,
    }
}

/// Convert GraphQL discount status to domain type (for single discount query).
const fn convert_discount_status_single(
    status: &queries::get_discount_code::DiscountStatus,
) -> DiscountStatus {
    match status {
        queries::get_discount_code::DiscountStatus::ACTIVE
        | queries::get_discount_code::DiscountStatus::Other(_) => DiscountStatus::Active,
        queries::get_discount_code::DiscountStatus::EXPIRED => DiscountStatus::Expired,
        queries::get_discount_code::DiscountStatus::SCHEDULED => DiscountStatus::Scheduled,
    }
}

/// Convert GraphQL payout status to domain type.
const fn convert_payout_status(
    status: &queries::get_payouts::ShopifyPaymentsPayoutStatus,
) -> PayoutStatus {
    match status {
        queries::get_payouts::ShopifyPaymentsPayoutStatus::SCHEDULED
        | queries::get_payouts::ShopifyPaymentsPayoutStatus::Other(_) => PayoutStatus::Scheduled,
        queries::get_payouts::ShopifyPaymentsPayoutStatus::IN_TRANSIT => PayoutStatus::InTransit,
        queries::get_payouts::ShopifyPaymentsPayoutStatus::PAID => PayoutStatus::Paid,
        queries::get_payouts::ShopifyPaymentsPayoutStatus::FAILED => PayoutStatus::Failed,
        queries::get_payouts::ShopifyPaymentsPayoutStatus::CANCELED => PayoutStatus::Canceled,
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

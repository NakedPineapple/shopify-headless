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
        AdminProduct, AdminProductConnection, AdminProductVariant, CalculatedOrder, Collection,
        CollectionConnection, CollectionProduct, CollectionWithProducts, Customer,
        CustomerConnection, CustomerSegment, DiscountCode, DiscountCodeConnection,
        DiscountCombinesWith, DiscountListConnection, DiscountListItem, DiscountMethod,
        DiscountMinimumRequirement, DiscountSortKey, DiscountStatus, DiscountType, DiscountValue,
        FulfillmentHoldInput, FulfillmentHoldReason, FulfillmentOrder, FulfillmentOrderLineItem,
        GiftCard, GiftCardConfiguration, GiftCardConnection, GiftCardDetail, GiftCardRecipient,
        GiftCardSortKey, GiftCardTransaction, Image, InventoryItem, InventoryItemConnection,
        InventoryLevel, InventoryLevelConnection, Location, LocationConnection, Money, Order,
        OrderConnection, OrderDetail, OrderEditAddShippingLineInput, OrderEditAppliedDiscountInput,
        OrderEditUpdateShippingLineInput, PageInfo, Payout, PayoutConnection, PayoutStatus,
        RefundCreateInput, RefundRestockType, ReturnCreateInput, StagedUploadTarget,
        SuggestedRefundLineItem, SuggestedRefundResult,
    },
};

mod conversions;
pub mod queries;

use conversions::{
    convert_calculated_order, convert_customer, convert_customer_connection,
    convert_fulfillment_orders, convert_inventory_item_connection,
    convert_inventory_level_connection, convert_location_connection, convert_order,
    convert_order_connection, convert_order_list_connection, convert_product,
    convert_product_connection, convert_single_inventory_item,
};
use queries::{
    ActivateInventory, CollectionAddProductsV2, CollectionCreate, CollectionDelete,
    CollectionRemoveProducts, CollectionUpdate, CollectionUpdateFields, CollectionUpdateSortOrder,
    CustomerAddressCreate, CustomerAddressDelete, CustomerAddressUpdate, CustomerCreate,
    CustomerDelete, CustomerEmailMarketingConsentUpdate, CustomerGenerateAccountActivationUrl,
    CustomerMerge, CustomerSendAccountInviteEmail, CustomerSmsMarketingConsentUpdate,
    CustomerUpdate, CustomerUpdateDefaultAddress, DeactivateInventory, DiscountAutomaticActivate,
    DiscountAutomaticDeactivate, DiscountAutomaticDelete, DiscountCodeActivate,
    DiscountCodeBasicCreate, DiscountCodeBasicUpdate, DiscountCodeBulkActivate,
    DiscountCodeBulkDeactivate, DiscountCodeBulkDelete, DiscountCodeDeactivate, DiscountCodeDelete,
    FileDelete, FileUpdate, FulfillmentCreate, FulfillmentOrderHold, FulfillmentOrderReleaseHold,
    FulfillmentTrackingInfoUpdate, GetCollection, GetCollectionWithProducts, GetCollections,
    GetCustomer, GetCustomerSegments, GetCustomers, GetDiscountCode, GetDiscountCodes,
    GetDiscountNodes, GetFulfillmentOrders, GetGiftCardConfiguration, GetGiftCardDetail,
    GetGiftCards, GetGiftCardsCount, GetInventoryItem, GetInventoryItems, GetInventoryLevels,
    GetLocations, GetOrder, GetOrderDetail, GetOrders, GetPayout, GetPayouts, GetProduct,
    GetProducts, GetPublications, GiftCardCreate, GiftCardCredit, GiftCardDeactivate,
    GiftCardDebit, GiftCardSendNotificationToCustomer, GiftCardSendNotificationToRecipient,
    GiftCardUpdate, InventoryAdjustQuantities, InventorySetQuantities, MoveInventory, OrderCancel,
    OrderCapture, OrderClose, OrderEditAddCustomItem, OrderEditAddLineItemDiscount,
    OrderEditAddShippingLine, OrderEditAddVariant, OrderEditBegin, OrderEditCommit,
    OrderEditRemoveDiscount, OrderEditRemoveShippingLine, OrderEditSetQuantity,
    OrderEditUpdateDiscount, OrderEditUpdateShippingLine, OrderMarkAsPaid, OrderOpen, OrderTagsAdd,
    OrderTagsRemove, OrderUpdate, ProductCreate, ProductDelete, ProductReorderMedia,
    ProductSetMedia, ProductUpdate, ProductVariantsBulkUpdate, PublishablePublish,
    PublishableUnpublish, RefundCreate, ReturnCreate, StagedUploadsCreate, SuggestedRefund,
    TagsAdd, TagsRemove, UpdateInventoryItem,
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
    /// Execute a raw GraphQL query and return the JSON response.
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

        // Check for GraphQL errors
        if let Some(errors) = response.get("errors") {
            let error_messages: Vec<String> = errors
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            if !error_messages.is_empty() {
                return Err(AdminShopifyError::GraphQL(
                    error_messages
                        .into_iter()
                        .map(|msg| GraphQLError {
                            message: msg,
                            locations: vec![],
                            path: vec![],
                        })
                        .collect(),
                ));
            }
        }

        Ok(response)
    }

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

        // Convert status string to enum
        let status = input.status.map(|s| match s.to_uppercase().as_str() {
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
                status,
                // All other fields set to None - skip_none should handle this
                // but we rely on the caller to provide all values to avoid nulls
                category: None,
                claim_ownership: None,
                collections_to_join: None,
                collections_to_leave: None,
                combined_listing_role: None,
                gift_card: None,
                gift_card_template_suffix: None,
                handle: None,
                metafields: None,
                product_options: None,
                redirect_new_handle: None,
                requires_selling_plan: None,
                seo: None,
                template_suffix: None,
            },
        };

        let response = self.execute::<ProductUpdate>(variables).await?;

        if let Some(payload) = response.product_update {
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
            use super::types::{
                CollectionRule, CollectionRuleSet, CollectionSeo, Publication, ResourcePublication,
            };

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
                publications: c
                    .resource_publications_v2
                    .edges
                    .into_iter()
                    .map(|e| ResourcePublication {
                        publication: Publication {
                            id: e.node.publication.id.clone(),
                            #[allow(deprecated)]
                            name: e
                                .node
                                .publication
                                .catalog
                                .map(|c| c.title)
                                .unwrap_or(e.node.publication.name),
                        },
                        is_published: e.node.is_published,
                    })
                    .collect(),
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
                    publications: vec![],
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
        use queries::collection_update_fields::{CollectionSortOrder, Variables};

        // Convert sort order string to enum (default to MANUAL if not provided)
        let sort_order_enum = match sort_order.unwrap_or("MANUAL") {
            "BEST_SELLING" => CollectionSortOrder::BEST_SELLING,
            "ALPHA_ASC" => CollectionSortOrder::ALPHA_ASC,
            "ALPHA_DESC" => CollectionSortOrder::ALPHA_DESC,
            "PRICE_ASC" => CollectionSortOrder::PRICE_ASC,
            "PRICE_DESC" => CollectionSortOrder::PRICE_DESC,
            "CREATED_DESC" => CollectionSortOrder::CREATED_DESC,
            "CREATED" => CollectionSortOrder::CREATED,
            _ => CollectionSortOrder::MANUAL,
        };

        let variables = Variables {
            id: id.to_string(),
            title: title.unwrap_or("").to_string(),
            description_html: description_html.unwrap_or("").to_string(),
            sort_order: sort_order_enum,
            seo_title: seo_title.unwrap_or("").to_string(),
            seo_description: seo_description.unwrap_or("").to_string(),
        };

        let response = self.execute::<CollectionUpdateFields>(variables).await?;

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

    /// Update only the sort order of a collection.
    ///
    /// This uses a focused mutation that only sends the sort order field,
    /// avoiding the `graphql_client` `skip_none` bug.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn update_collection_sort_order(
        &self,
        id: &str,
        sort_order: &str,
    ) -> Result<(), AdminShopifyError> {
        use queries::collection_update_sort_order::{CollectionSortOrder, Variables};

        let sort_order_enum = match sort_order {
            "BEST_SELLING" => CollectionSortOrder::BEST_SELLING,
            "ALPHA_ASC" => CollectionSortOrder::ALPHA_ASC,
            "ALPHA_DESC" => CollectionSortOrder::ALPHA_DESC,
            "PRICE_ASC" => CollectionSortOrder::PRICE_ASC,
            "PRICE_DESC" => CollectionSortOrder::PRICE_DESC,
            "CREATED_DESC" => CollectionSortOrder::CREATED_DESC,
            "CREATED" => CollectionSortOrder::CREATED,
            "MANUAL" => CollectionSortOrder::MANUAL,
            _ => {
                return Err(AdminShopifyError::UserError(format!(
                    "Invalid sort order: {sort_order}"
                )));
            }
        };

        let variables = Variables {
            id: id.to_string(),
            sort_order: sort_order_enum,
        };

        let response = self.execute::<CollectionUpdateSortOrder>(variables).await?;

        if let Some(payload) = response.collection_update
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

    /// Update a collection's image.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn update_collection_image(
        &self,
        id: &str,
        image_url: &str,
        alt_text: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        // Build image input
        let image_obj = alt_text.map_or_else(
            || serde_json::json!({ "src": image_url }),
            |alt| serde_json::json!({ "src": image_url, "altText": alt }),
        );

        let query = r"
            mutation CollectionUpdateImage($input: CollectionInput!) {
                collectionUpdate(input: $input) {
                    collection {
                        id
                        image {
                            id
                            url
                        }
                    }
                    userErrors {
                        field
                        message
                    }
                }
            }
        ";

        let body = serde_json::json!({
            "query": query,
            "variables": {
                "input": {
                    "id": id,
                    "image": image_obj
                }
            }
        });

        let response = self.execute_raw_graphql(body).await?;

        // Check for user errors
        if let Some(errors) = response
            .get("collectionUpdate")
            .and_then(|p| p.get("userErrors"))
            .and_then(|e| e.as_array())
        {
            let error_messages: Vec<String> = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .map(String::from)
                .collect();

            if !error_messages.is_empty() {
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }
        }

        Ok(())
    }

    /// Delete a collection's image.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn delete_collection_image(&self, id: &str) -> Result<(), AdminShopifyError> {
        let query = r"
            mutation CollectionDeleteImage($input: CollectionInput!) {
                collectionUpdate(input: $input) {
                    collection {
                        id
                    }
                    userErrors {
                        field
                        message
                    }
                }
            }
        ";

        // Setting image to null removes it
        let body = serde_json::json!({
            "query": query,
            "variables": {
                "input": {
                    "id": id,
                    "image": serde_json::Value::Null
                }
            }
        });

        let response = self.execute_raw_graphql(body).await?;

        // Check for user errors
        if let Some(errors) = response
            .get("collectionUpdate")
            .and_then(|p| p.get("userErrors"))
            .and_then(|e| e.as_array())
        {
            let error_messages: Vec<String> = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .map(String::from)
                .collect();

            if !error_messages.is_empty() {
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }
        }

        Ok(())
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
        use super::types::{
            CollectionRule, CollectionRuleSet, CollectionSeo, Publication, ResourcePublication,
        };

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
                publications: c
                    .resource_publications_v2
                    .edges
                    .into_iter()
                    .map(|e| ResourcePublication {
                        publication: Publication {
                            id: e.node.publication.id.clone(),
                            #[allow(deprecated)]
                            name: e
                                .node
                                .publication
                                .catalog
                                .map(|c| c.title)
                                .unwrap_or(e.node.publication.name),
                        },
                        is_published: e.node.is_published,
                    })
                    .collect(),
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

    /// Reorder products in a collection (manual sort only).
    ///
    /// # Arguments
    ///
    /// * `collection_id` - The collection GID
    /// * `moves` - List of (`product_id`, `new_position`) tuples
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn reorder_collection_products(
        &self,
        collection_id: &str,
        moves: Vec<(String, i64)>,
    ) -> Result<(), AdminShopifyError> {
        let query = r"
            mutation CollectionReorderProducts($id: ID!, $moves: [MoveInput!]!) {
                collectionReorderProducts(id: $id, moves: $moves) {
                    job {
                        id
                        done
                    }
                    userErrors {
                        field
                        message
                    }
                }
            }
        ";

        let moves_input: Vec<serde_json::Value> = moves
            .into_iter()
            .map(|(id, new_position)| {
                serde_json::json!({
                    "id": id,
                    "newPosition": new_position
                })
            })
            .collect();

        let body = serde_json::json!({
            "query": query,
            "variables": {
                "id": collection_id,
                "moves": moves_input
            }
        });

        let response = self.execute_raw_graphql(body).await?;

        // Check for user errors
        if let Some(errors) = response
            .get("collectionReorderProducts")
            .and_then(|p| p.get("userErrors"))
            .and_then(|e| e.as_array())
        {
            let error_messages: Vec<String> = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .map(String::from)
                .collect();

            if !error_messages.is_empty() {
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }
        }

        Ok(())
    }

    /// Get all publications (sales channels) for the shop.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_publications(
        &self,
    ) -> Result<Vec<super::types::Publication>, AdminShopifyError> {
        let variables = queries::get_publications::Variables {};
        let response = self.execute::<GetPublications>(variables).await?;

        Ok(response
            .publications
            .edges
            .into_iter()
            .map(|e| {
                let id = e.node.id;
                #[allow(deprecated)]
                let name = e.node.catalog.map(|c| c.title).unwrap_or(e.node.name);
                super::types::Publication { id, name }
            })
            .collect())
    }

    /// Publish a collection to specified publications.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn publish_collection(
        &self,
        collection_id: &str,
        publication_ids: &[String],
    ) -> Result<(), AdminShopifyError> {
        if publication_ids.is_empty() {
            return Ok(());
        }

        let variables = queries::publishable_publish::Variables {
            id: collection_id.to_string(),
            input: publication_ids
                .iter()
                .map(|pub_id| queries::publishable_publish::PublicationInput {
                    publication_id: Some(pub_id.clone()),
                    publish_date: None,
                })
                .collect(),
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
                    format!("{field}: {}", e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Unpublish a collection from specified publications.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn unpublish_collection(
        &self,
        collection_id: &str,
        publication_ids: &[String],
    ) -> Result<(), AdminShopifyError> {
        if publication_ids.is_empty() {
            return Ok(());
        }

        let variables = queries::publishable_unpublish::Variables {
            id: collection_id.to_string(),
            input: publication_ids
                .iter()
                .map(|pub_id| queries::publishable_unpublish::PublicationInput {
                    publication_id: Some(pub_id.clone()),
                    publish_date: None,
                })
                .collect(),
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
                    format!("{field}: {}", e.message)
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

    /// Get detailed order information for the order detail page.
    ///
    /// Returns extended order data including transactions, fulfillments, refunds,
    /// returns, timeline events, and customer info.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID (e.g., `gid://shopify/Order/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn get_order_detail(
        &self,
        id: &str,
    ) -> Result<Option<queries::get_order_detail::GetOrderDetailOrder>, AdminShopifyError> {
        let variables = queries::get_order_detail::Variables {
            id: id.to_string(),
            line_item_count: Some(100),
            fulfillment_count: Some(50),
            transaction_count: Some(50),
            event_count: Some(100),
        };

        let response = self.execute::<GetOrderDetail>(variables).await?;

        Ok(response.order)
    }

    /// Get a paginated list of orders with extended fields for data table display.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of orders to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query (Shopify query syntax)
    /// * `sort_key` - Optional sort key
    /// * `reverse` - Whether to reverse the sort order
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self))]
    pub async fn get_orders_list(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
        sort_key: Option<crate::shopify::types::OrderSortKey>,
        reverse: bool,
    ) -> Result<crate::shopify::types::OrderListConnection, AdminShopifyError> {
        let variables = queries::get_orders::Variables {
            first: Some(first),
            after,
            query,
            sort_key: sort_key.map(|k| match k {
                crate::shopify::types::OrderSortKey::OrderNumber => {
                    queries::get_orders::OrderSortKeys::ORDER_NUMBER
                }
                crate::shopify::types::OrderSortKey::TotalPrice => {
                    queries::get_orders::OrderSortKeys::TOTAL_PRICE
                }
                crate::shopify::types::OrderSortKey::CreatedAt => {
                    queries::get_orders::OrderSortKeys::CREATED_AT
                }
                crate::shopify::types::OrderSortKey::ProcessedAt => {
                    queries::get_orders::OrderSortKeys::PROCESSED_AT
                }
                crate::shopify::types::OrderSortKey::UpdatedAt => {
                    queries::get_orders::OrderSortKeys::UPDATED_AT
                }
                crate::shopify::types::OrderSortKey::CustomerName => {
                    queries::get_orders::OrderSortKeys::CUSTOMER_NAME
                }
                crate::shopify::types::OrderSortKey::FinancialStatus => {
                    queries::get_orders::OrderSortKeys::FINANCIAL_STATUS
                }
                crate::shopify::types::OrderSortKey::FulfillmentStatus => {
                    queries::get_orders::OrderSortKeys::FULFILLMENT_STATUS
                }
                crate::shopify::types::OrderSortKey::Destination => {
                    queries::get_orders::OrderSortKeys::DESTINATION
                }
                crate::shopify::types::OrderSortKey::Id => queries::get_orders::OrderSortKeys::ID,
            }),
            reverse: Some(reverse),
        };

        let response = self.execute::<GetOrders>(variables).await?;

        Ok(convert_order_list_connection(response.orders))
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

        let cancel_reason = reason.map_or(OrderCancelReason::OTHER, |r| {
            match r.to_uppercase().as_str() {
                "CUSTOMER" => OrderCancelReason::CUSTOMER,
                "FRAUD" => OrderCancelReason::FRAUD,
                "INVENTORY" => OrderCancelReason::INVENTORY,
                "DECLINED" => OrderCancelReason::DECLINED,
                _ => OrderCancelReason::OTHER,
            }
        });

        let variables = Variables {
            order_id: id.to_string(),
            reason: cancel_reason,
            notify_customer: Some(notify_customer),
            refund: Some(refund),
            restock,
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

    /// Archive (close) an order.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn archive_order(&self, id: &str) -> Result<(), AdminShopifyError> {
        use queries::order_close::{OrderCloseInput, Variables};

        let variables = Variables {
            input: OrderCloseInput { id: id.to_string() },
        };

        let response = self.execute::<OrderClose>(variables).await?;

        if let Some(payload) = response.order_close
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

    /// Unarchive (reopen) an order.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn unarchive_order(&self, id: &str) -> Result<(), AdminShopifyError> {
        use queries::order_open::{OrderOpenInput, Variables};

        let variables = Variables {
            input: OrderOpenInput { id: id.to_string() },
        };

        let response = self.execute::<OrderOpen>(variables).await?;

        if let Some(payload) = response.order_open
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

    /// Capture payment on an order.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    /// * `parent_transaction_id` - ID of the authorized transaction to capture
    /// * `amount` - Amount to capture (required)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn capture_order_payment(
        &self,
        id: &str,
        parent_transaction_id: &str,
        amount: &str,
    ) -> Result<(), AdminShopifyError> {
        use queries::order_capture::{OrderCaptureInput, Variables};

        let variables = Variables {
            input: OrderCaptureInput {
                id: id.to_string(),
                amount: amount.to_string(),
                parent_transaction_id: parent_transaction_id.to_string(),
                currency: None,
                final_capture: None,
            },
        };

        let response = self.execute::<OrderCapture>(variables).await?;

        if let Some(payload) = response.order_capture
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

    /// Add tags to an order.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    /// * `tags` - Tags to add
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn add_tags_to_order(
        &self,
        id: &str,
        tags: &[String],
    ) -> Result<Vec<String>, AdminShopifyError> {
        use queries::order_tags_add::Variables;

        let variables = Variables {
            id: id.to_string(),
            tags: tags.to_vec(),
        };

        let response = self.execute::<OrderTagsAdd>(variables).await?;

        if let Some(payload) = response.tags_add {
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

            // Tags were added successfully
            return Ok(vec![]);
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Tags add failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Remove tags from an order.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    /// * `tags` - Tags to remove
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn remove_tags_from_order(
        &self,
        id: &str,
        tags: &[String],
    ) -> Result<Vec<String>, AdminShopifyError> {
        use queries::order_tags_remove::Variables;

        let variables = Variables {
            id: id.to_string(),
            tags: tags.to_vec(),
        };

        let response = self.execute::<OrderTagsRemove>(variables).await?;

        if let Some(payload) = response.tags_remove {
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

            // Tags were removed successfully
            return Ok(vec![]);
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Tags remove failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    // =========================================================================
    // Order Edit methods
    // =========================================================================

    /// Begin an order edit session.
    ///
    /// This starts a new order edit session and returns a `CalculatedOrder`
    /// which tracks the proposed changes until they are committed.
    ///
    /// # Arguments
    ///
    /// * `order_id` - Shopify order ID (e.g., `gid://shopify/Order/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn order_edit_begin(
        &self,
        order_id: &str,
    ) -> Result<CalculatedOrder, AdminShopifyError> {
        let variables = queries::order_edit_begin::Variables {
            id: order_id.to_string(),
        };

        let response = self.execute::<OrderEditBegin>(variables).await?;

        if let Some(payload) = response.order_edit_begin {
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

            if let Some(calc_order) = payload.calculated_order {
                return Ok(convert_calculated_order(calc_order));
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Order edit begin failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Add a product variant to an order edit.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `variant_id` - Shopify product variant ID
    /// * `quantity` - Quantity to add
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(calc_order_id = %calculated_order_id, variant_id = %variant_id))]
    pub async fn order_edit_add_variant(
        &self,
        calculated_order_id: &str,
        variant_id: &str,
        quantity: i64,
    ) -> Result<(), AdminShopifyError> {
        let variables = queries::order_edit_add_variant::Variables {
            id: calculated_order_id.to_string(),
            variant_id: variant_id.to_string(),
            quantity,
            location_id: None,
            allow_duplicates: Some(false),
        };

        let response = self.execute::<OrderEditAddVariant>(variables).await?;

        if let Some(payload) = response.order_edit_add_variant {
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
            message: "Order edit add variant failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Add a custom line item to an order edit.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `title` - Title for the custom item
    /// * `quantity` - Quantity to add
    /// * `price` - Unit price for the item
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(calc_order_id = %calculated_order_id, title = %title))]
    pub async fn order_edit_add_custom_item(
        &self,
        calculated_order_id: &str,
        title: &str,
        quantity: i64,
        price: &Money,
        taxable: bool,
        requires_shipping: bool,
    ) -> Result<(), AdminShopifyError> {
        use queries::order_edit_add_custom_item::MoneyInput;

        let variables = queries::order_edit_add_custom_item::Variables {
            id: calculated_order_id.to_string(),
            title: title.to_string(),
            quantity,
            price: MoneyInput {
                amount: price.amount.clone(),
                currency_code: queries::order_edit_add_custom_item::CurrencyCode::USD,
            },
            location_id: None,
            taxable: Some(taxable),
            requires_shipping: Some(requires_shipping),
        };

        let response = self.execute::<OrderEditAddCustomItem>(variables).await?;

        if let Some(payload) = response.order_edit_add_custom_item {
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
            message: "Order edit add custom item failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Set the quantity of a line item in an order edit.
    ///
    /// Set quantity to 0 to remove the item.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `line_item_id` - The calculated line item ID
    /// * `quantity` - New quantity (0 to remove)
    /// * `restock` - Whether to restock items when reducing quantity
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(calc_order_id = %calculated_order_id, line_item_id = %line_item_id))]
    pub async fn order_edit_set_quantity(
        &self,
        calculated_order_id: &str,
        line_item_id: &str,
        quantity: i64,
        restock: bool,
    ) -> Result<(), AdminShopifyError> {
        let variables = queries::order_edit_set_quantity::Variables {
            id: calculated_order_id.to_string(),
            line_item_id: line_item_id.to_string(),
            quantity,
            restock: Some(restock),
        };

        let response = self.execute::<OrderEditSetQuantity>(variables).await?;

        if let Some(payload) = response.order_edit_set_quantity {
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
            message: "Order edit set quantity failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Add a discount to a line item in an order edit.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `line_item_id` - The calculated line item ID
    /// * `discount` - The discount to apply
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, discount), fields(calc_order_id = %calculated_order_id, line_item_id = %line_item_id))]
    pub async fn order_edit_add_line_item_discount(
        &self,
        calculated_order_id: &str,
        line_item_id: &str,
        discount: &OrderEditAppliedDiscountInput,
    ) -> Result<(), AdminShopifyError> {
        use queries::order_edit_add_line_item_discount::{
            MoneyInput, OrderEditAppliedDiscountInput as GqlDiscount,
        };

        let gql_discount = GqlDiscount {
            description: discount.description.clone(),
            fixed_value: discount.fixed_value.as_ref().map(|m| MoneyInput {
                amount: m.amount.clone(),
                currency_code: queries::order_edit_add_line_item_discount::CurrencyCode::USD,
            }),
            percent_value: discount.percent_value,
        };

        let variables = queries::order_edit_add_line_item_discount::Variables {
            id: calculated_order_id.to_string(),
            line_item_id: line_item_id.to_string(),
            discount: gql_discount,
        };

        let response = self
            .execute::<OrderEditAddLineItemDiscount>(variables)
            .await?;

        if let Some(payload) = response.order_edit_add_line_item_discount {
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
            message: "Order edit add line item discount failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update an existing discount in an order edit.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `discount_application_id` - The discount application ID to update
    /// * `discount` - The new discount values
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, discount), fields(calc_order_id = %calculated_order_id, discount_id = %discount_application_id))]
    pub async fn order_edit_update_discount(
        &self,
        calculated_order_id: &str,
        discount_application_id: &str,
        discount: &OrderEditAppliedDiscountInput,
    ) -> Result<(), AdminShopifyError> {
        use queries::order_edit_update_discount::{
            MoneyInput, OrderEditAppliedDiscountInput as GqlDiscount,
        };

        let gql_discount = GqlDiscount {
            description: discount.description.clone(),
            fixed_value: discount.fixed_value.as_ref().map(|m| MoneyInput {
                amount: m.amount.clone(),
                currency_code: queries::order_edit_update_discount::CurrencyCode::USD,
            }),
            percent_value: discount.percent_value,
        };

        let variables = queries::order_edit_update_discount::Variables {
            id: calculated_order_id.to_string(),
            discount_application_id: discount_application_id.to_string(),
            discount: gql_discount,
        };

        let response = self.execute::<OrderEditUpdateDiscount>(variables).await?;

        if let Some(payload) = response.order_edit_update_discount {
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
            message: "Order edit update discount failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Remove a discount from an order edit.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `discount_application_id` - The discount application ID to remove
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(calc_order_id = %calculated_order_id, discount_id = %discount_application_id))]
    pub async fn order_edit_remove_discount(
        &self,
        calculated_order_id: &str,
        discount_application_id: &str,
    ) -> Result<(), AdminShopifyError> {
        let variables = queries::order_edit_remove_discount::Variables {
            id: calculated_order_id.to_string(),
            discount_application_id: discount_application_id.to_string(),
        };

        let response = self.execute::<OrderEditRemoveDiscount>(variables).await?;

        if let Some(payload) = response.order_edit_remove_discount {
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
            message: "Order edit remove discount failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Add a shipping line to an order edit.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `input` - Shipping line details (title and price)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input), fields(calc_order_id = %calculated_order_id))]
    pub async fn order_edit_add_shipping_line(
        &self,
        calculated_order_id: &str,
        input: &OrderEditAddShippingLineInput,
    ) -> Result<(), AdminShopifyError> {
        use queries::order_edit_add_shipping_line::{
            MoneyInput, OrderEditAddShippingLineInput as GqlInput,
        };

        let gql_input = GqlInput {
            title: input.title.clone(),
            price: MoneyInput {
                amount: input.price.amount.clone(),
                currency_code: queries::order_edit_add_shipping_line::CurrencyCode::USD,
            },
        };

        let variables = queries::order_edit_add_shipping_line::Variables {
            id: calculated_order_id.to_string(),
            shipping_line: gql_input,
        };

        let response = self.execute::<OrderEditAddShippingLine>(variables).await?;

        if let Some(payload) = response.order_edit_add_shipping_line {
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
            message: "Order edit add shipping line failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update a shipping line in an order edit.
    ///
    /// Only staged (newly added) shipping lines can be updated.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `shipping_line_id` - The shipping line ID to update
    /// * `input` - New shipping line details
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input), fields(calc_order_id = %calculated_order_id, shipping_line_id = %shipping_line_id))]
    pub async fn order_edit_update_shipping_line(
        &self,
        calculated_order_id: &str,
        shipping_line_id: &str,
        input: &OrderEditUpdateShippingLineInput,
    ) -> Result<(), AdminShopifyError> {
        use queries::order_edit_update_shipping_line::{
            MoneyInput, OrderEditUpdateShippingLineInput as GqlInput,
        };

        let gql_input = GqlInput {
            title: input.title.clone(),
            price: input.price.as_ref().map(|p| MoneyInput {
                amount: p.amount.clone(),
                currency_code: queries::order_edit_update_shipping_line::CurrencyCode::USD,
            }),
        };

        let variables = queries::order_edit_update_shipping_line::Variables {
            id: calculated_order_id.to_string(),
            shipping_line_id: shipping_line_id.to_string(),
            shipping_line: gql_input,
        };

        let response = self
            .execute::<OrderEditUpdateShippingLine>(variables)
            .await?;

        if let Some(payload) = response.order_edit_update_shipping_line {
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
            message: "Order edit update shipping line failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Remove a shipping line from an order edit.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `shipping_line_id` - The shipping line ID to remove
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(calc_order_id = %calculated_order_id, shipping_line_id = %shipping_line_id))]
    pub async fn order_edit_remove_shipping_line(
        &self,
        calculated_order_id: &str,
        shipping_line_id: &str,
    ) -> Result<(), AdminShopifyError> {
        let variables = queries::order_edit_remove_shipping_line::Variables {
            id: calculated_order_id.to_string(),
            shipping_line_id: shipping_line_id.to_string(),
        };

        let response = self
            .execute::<OrderEditRemoveShippingLine>(variables)
            .await?;

        if let Some(payload) = response.order_edit_remove_shipping_line {
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
            message: "Order edit remove shipping line failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Commit an order edit, finalizing all changes.
    ///
    /// This applies all staged changes to the order. If the edit changes
    /// the total, the customer may need to pay a balance or receive a refund.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `notify_customer` - Whether to notify the customer about the changes
    /// * `staff_note` - Optional internal note about the edit
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(calc_order_id = %calculated_order_id))]
    pub async fn order_edit_commit(
        &self,
        calculated_order_id: &str,
        notify_customer: bool,
        staff_note: Option<&str>,
    ) -> Result<String, AdminShopifyError> {
        let variables = queries::order_edit_commit::Variables {
            id: calculated_order_id.to_string(),
            notify_customer: Some(notify_customer),
            staff_note: staff_note.map(String::from),
        };

        let response = self.execute::<OrderEditCommit>(variables).await?;

        if let Some(payload) = response.order_edit_commit {
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

            if let Some(order) = payload.order {
                return Ok(order.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Order edit commit failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
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
        let variables = queries::get_fulfillment_orders::Variables {
            order_id: order_id.to_string(),
        };

        let response = self.execute::<GetFulfillmentOrders>(variables).await?;

        Ok(convert_fulfillment_orders(response.order))
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
        use queries::fulfillment_create::{
            FulfillmentInput, FulfillmentOrderLineItemsInput, FulfillmentTrackingInput, Variables,
        };

        // Build tracking info if any tracking details provided
        let tracking_info =
            if tracking_company.is_some() || tracking_number.is_some() || tracking_url.is_some() {
                Some(FulfillmentTrackingInput {
                    company: tracking_company.map(String::from),
                    number: tracking_number.map(String::from),
                    url: tracking_url.map(String::from),
                    numbers: None,
                    urls: None,
                })
            } else {
                None
            };

        let variables = Variables {
            fulfillment: FulfillmentInput {
                line_items_by_fulfillment_order: vec![FulfillmentOrderLineItemsInput {
                    fulfillment_order_id: fulfillment_order_id.to_string(),
                    fulfillment_order_line_items: None, // Fulfill all items
                }],
                tracking_info,
                notify_customer: Some(true),
                origin_address: None,
            },
        };

        let response = self.execute::<FulfillmentCreate>(variables).await?;

        if let Some(payload) = response.fulfillment_create {
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

            if let Some(fulfillment) = payload.fulfillment {
                return Ok(fulfillment.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No fulfillment returned from create".to_string(),
            locations: vec![],
            path: vec![],
        }]))
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
        use queries::fulfillment_tracking_info_update::{FulfillmentTrackingInput, Variables};

        let variables = Variables {
            fulfillment_id: fulfillment_id.to_string(),
            tracking_info_input: FulfillmentTrackingInput {
                company: tracking_company.map(String::from),
                number: tracking_number.map(String::from),
                url: tracking_url.map(String::from),
                numbers: None,
                urls: None,
            },
        };

        let response = self
            .execute::<FulfillmentTrackingInfoUpdate>(variables)
            .await?;

        if let Some(payload) = response.fulfillment_tracking_info_update
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
    #[instrument(skip(self, input), fields(order_id = %order_id))]
    pub async fn create_refund(
        &self,
        order_id: &str,
        input: RefundCreateInput,
    ) -> Result<String, AdminShopifyError> {
        use queries::refund_create::{
            RefundInput, RefundLineItemInput as GqlRefundLineItemInput, RefundLineItemRestockType,
            ShippingRefundInput, Variables,
        };

        // Convert line items
        let refund_line_items: Vec<GqlRefundLineItemInput> = input
            .line_items
            .into_iter()
            .map(|item| GqlRefundLineItemInput {
                line_item_id: item.line_item_id,
                quantity: item.quantity,
                restock_type: Some(match item.restock_type {
                    RefundRestockType::Return => RefundLineItemRestockType::RETURN,
                    RefundRestockType::Cancel => RefundLineItemRestockType::CANCEL,
                    RefundRestockType::NoRestock => RefundLineItemRestockType::NO_RESTOCK,
                }),
                location_id: item.location_id,
            })
            .collect();

        // Build shipping refund input if needed
        let shipping = if input.full_shipping_refund || input.shipping_amount.is_some() {
            Some(ShippingRefundInput {
                full_refund: Some(input.full_shipping_refund),
                amount: input.shipping_amount,
            })
        } else {
            None
        };

        let variables = Variables {
            input: RefundInput {
                order_id: order_id.to_string(),
                note: input.note,
                notify: Some(input.notify),
                refund_line_items: Some(refund_line_items),
                shipping,
                currency: None,
                processed_at: None,
                refund_duties: None,
                transactions: None,
                refund_methods: None,
                discrepancy_reason: None,
                allow_over_refunding: None,
            },
        };

        let response = self.execute::<RefundCreate>(variables).await?;

        if let Some(payload) = response.refund_create {
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

            if let Some(refund) = payload.refund {
                return Ok(refund.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No refund returned from create".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Get suggested refund calculation for an order.
    ///
    /// # Arguments
    ///
    /// * `order_id` - Shopify order ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or the order is not found.
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn get_suggested_refund(
        &self,
        order_id: &str,
    ) -> Result<SuggestedRefundResult, AdminShopifyError> {
        let variables = queries::suggested_refund::Variables {
            order_id: order_id.to_string(),
        };

        let response = self.execute::<SuggestedRefund>(variables).await?;

        let order = response.order.ok_or_else(|| {
            AdminShopifyError::GraphQL(vec![GraphQLError {
                message: "Order not found".to_string(),
                locations: vec![],
                path: vec![],
            }])
        })?;

        let suggested = order.suggested_refund.ok_or_else(|| {
            AdminShopifyError::GraphQL(vec![GraphQLError {
                message: "No suggested refund available".to_string(),
                locations: vec![],
                path: vec![],
            }])
        })?;

        let amount = suggested.amount_set.shop_money.amount.clone();
        let currency_code = format!("{:?}", suggested.amount_set.shop_money.currency_code);
        let subtotal = suggested.subtotal_set.shop_money.amount.clone();
        let total_tax = suggested.total_tax_set.shop_money.amount.clone();

        let line_items = suggested
            .refund_line_items
            .into_iter()
            .map(|item| SuggestedRefundLineItem {
                line_item_id: item.line_item.id,
                title: item.line_item.title,
                original_quantity: item.line_item.quantity,
                refund_quantity: item.quantity,
            })
            .collect();

        Ok(SuggestedRefundResult {
            amount,
            currency_code,
            subtotal,
            total_tax,
            line_items,
        })
    }

    // =========================================================================
    // Fulfillment hold methods
    // =========================================================================

    /// Hold a fulfillment order.
    ///
    /// Prevents the fulfillment order from being fulfilled until the hold is released.
    ///
    /// # Arguments
    ///
    /// * `fulfillment_order_id` - Fulfillment order ID to hold
    /// * `input` - Hold configuration (reason, notes, notify)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input), fields(fulfillment_order_id = %fulfillment_order_id))]
    pub async fn hold_fulfillment_order(
        &self,
        fulfillment_order_id: &str,
        input: FulfillmentHoldInput,
    ) -> Result<(), AdminShopifyError> {
        use queries::fulfillment_order_hold::{
            FulfillmentHoldReason as GqlReason, FulfillmentOrderHoldInput, Variables,
        };

        let reason = match input.reason {
            FulfillmentHoldReason::AwaitingPayment => GqlReason::AWAITING_PAYMENT,
            FulfillmentHoldReason::HighRiskOfFraud => GqlReason::HIGH_RISK_OF_FRAUD,
            FulfillmentHoldReason::IncorrectAddress => GqlReason::INCORRECT_ADDRESS,
            FulfillmentHoldReason::InventoryOutOfStock => GqlReason::INVENTORY_OUT_OF_STOCK,
            FulfillmentHoldReason::UnknownDeliveryDate => GqlReason::UNKNOWN_DELIVERY_DATE,
            FulfillmentHoldReason::AwaitingReturnItems => GqlReason::AWAITING_RETURN_ITEMS,
            FulfillmentHoldReason::Other => GqlReason::OTHER,
        };

        let variables = Variables {
            id: fulfillment_order_id.to_string(),
            fulfillment_hold: FulfillmentOrderHoldInput {
                reason,
                reason_notes: input.reason_notes,
                notify_merchant: Some(input.notify_merchant),
                external_id: None,
                handle: None,
                fulfillment_order_line_items: None,
            },
        };

        let response = self.execute::<FulfillmentOrderHold>(variables).await?;

        if let Some(payload) = response.fulfillment_order_hold
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

    /// Release a hold on a fulfillment order.
    ///
    /// # Arguments
    ///
    /// * `fulfillment_order_id` - Fulfillment order ID to release
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(fulfillment_order_id = %fulfillment_order_id))]
    pub async fn release_fulfillment_order_hold(
        &self,
        fulfillment_order_id: &str,
    ) -> Result<(), AdminShopifyError> {
        let variables = queries::fulfillment_order_release_hold::Variables {
            id: fulfillment_order_id.to_string(),
        };

        let response = self
            .execute::<FulfillmentOrderReleaseHold>(variables)
            .await?;

        if let Some(payload) = response.fulfillment_order_release_hold
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
    // Return methods
    // =========================================================================

    /// Create a return for an order.
    ///
    /// # Arguments
    ///
    /// * `order_id` - Shopify order ID
    /// * `input` - Return configuration (line items to return)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input), fields(order_id = %order_id))]
    pub async fn create_return(
        &self,
        order_id: &str,
        input: ReturnCreateInput,
    ) -> Result<String, AdminShopifyError> {
        use queries::return_create::{ReturnInput, ReturnLineItemInput, Variables};

        let return_line_items: Vec<ReturnLineItemInput> = input
            .line_items
            .into_iter()
            .map(|item| ReturnLineItemInput {
                fulfillment_line_item_id: item.fulfillment_line_item_id,
                quantity: item.quantity,
                return_reason_note: item.return_reason_note,
                return_reason_definition_id: None,
                restocking_fee: None,
            })
            .collect();

        let variables = Variables {
            return_input: ReturnInput {
                order_id: order_id.to_string(),
                return_line_items,
                requested_at: input.requested_at,
                exchange_line_items: None,
                return_shipping_fee: None,
            },
        };

        let response = self.execute::<ReturnCreate>(variables).await?;

        if let Some(payload) = response.return_create {
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

            if let Some(ret) = payload.return_ {
                return Ok(ret.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No return created".to_string(),
            locations: vec![],
            path: vec![],
        }]))
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
            order_count: Some(10),
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
    /// * `sort_key` - Optional sort key (NAME, LOCATION, `CREATED_AT`, etc.)
    /// * `reverse` - Whether to reverse the sort order
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self))]
    pub async fn get_customers(
        &self,
        params: super::types::CustomerListParams,
    ) -> Result<CustomerConnection, AdminShopifyError> {
        use super::types::CustomerSortKey;
        use queries::get_customers::CustomerSortKeys;

        // Check if we need client-side sorting
        let client_side_sort = params.sort_key.is_some_and(|sk| !sk.is_shopify_native());

        // Convert our sort key to the GraphQL enum (only for Shopify-native keys)
        let sort_key = params.sort_key.and_then(|sk| match sk {
            CustomerSortKey::CreatedAt => Some(CustomerSortKeys::CREATED_AT),
            CustomerSortKey::Id => Some(CustomerSortKeys::ID),
            CustomerSortKey::Location => Some(CustomerSortKeys::LOCATION),
            CustomerSortKey::Name => Some(CustomerSortKeys::NAME),
            CustomerSortKey::Relevance => Some(CustomerSortKeys::RELEVANCE),
            CustomerSortKey::UpdatedAt => Some(CustomerSortKeys::UPDATED_AT),
            // Client-side sorted keys - don't pass to API
            CustomerSortKey::AmountSpent | CustomerSortKey::OrdersCount => None,
        });

        let variables = queries::get_customers::Variables {
            first: params.first,
            after: params.after,
            query: params.query.clone(),
            sort_key,
            // Only apply reverse on server if not doing client-side sort
            reverse: Some(if client_side_sort {
                false
            } else {
                params.reverse
            }),
        };

        let response = self.execute::<GetCustomers>(variables).await?;
        let mut connection = convert_customer_connection(response.customers);

        // Apply client-side sorting if needed
        if client_side_sort && let Some(sk) = params.sort_key {
            sort_customers(&mut connection.customers, sk, params.reverse);
        }

        Ok(connection)
    }

    /// Create a new customer.
    ///
    /// # Arguments
    ///
    /// * `email` - Customer email address
    /// * `first_name` - Customer first name
    /// * `last_name` - Customer last name
    /// * `phone` - Optional phone number
    /// * `note` - Optional customer note
    /// * `tags` - Optional tags
    ///
    /// # Returns
    ///
    /// Returns the created customer's ID on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn create_customer(
        &self,
        email: &str,
        first_name: Option<&str>,
        last_name: Option<&str>,
        phone: Option<&str>,
        note: Option<&str>,
        tags: Vec<String>,
    ) -> Result<String, AdminShopifyError> {
        use queries::customer_create::{CustomerInput, Variables};

        let variables = Variables {
            input: CustomerInput {
                id: None,
                email: Some(email.to_string()),
                first_name: first_name.map(String::from),
                last_name: last_name.map(String::from),
                phone: phone.map(String::from),
                note: note.map(String::from),
                tags: Some(tags),
                // Other optional fields
                addresses: None,
                locale: None,
                metafields: None,
                multipass_identifier: None,
                sms_marketing_consent: None,
                email_marketing_consent: None,
                tax_exempt: None,
                tax_exemptions: None,
            },
        };

        let response: <CustomerCreate as GraphQLQuery>::ResponseData =
            self.execute::<CustomerCreate>(variables).await?;

        if let Some(payload) = response.customer_create {
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

            if let Some(customer) = payload.customer {
                return Ok(customer.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No customer returned from create".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update an existing customer.
    ///
    /// # Arguments
    ///
    /// * `id` - Customer ID
    /// * `email` - Optional new email
    /// * `first_name` - Optional new first name
    /// * `last_name` - Optional new last name
    /// * `phone` - Optional new phone
    /// * `note` - Optional new note
    /// * `tags` - Optional new tags (replaces existing)
    ///
    /// # Returns
    ///
    /// Returns the updated customer's ID on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, params))]
    pub async fn update_customer(
        &self,
        id: &str,
        params: super::types::CustomerUpdateParams,
    ) -> Result<String, AdminShopifyError> {
        use queries::customer_update::{CustomerInput, Variables};

        let variables = Variables {
            input: CustomerInput {
                id: Some(id.to_string()),
                email: params.email,
                first_name: params.first_name,
                last_name: params.last_name,
                phone: params.phone,
                note: params.note,
                tags: params.tags,
                addresses: None,
                locale: None,
                metafields: None,
                multipass_identifier: None,
                sms_marketing_consent: None,
                email_marketing_consent: None,
                tax_exempt: None,
                tax_exemptions: None,
            },
        };

        let response: <CustomerUpdate as GraphQLQuery>::ResponseData =
            self.execute::<CustomerUpdate>(variables).await?;

        if let Some(payload) = response.customer_update {
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

            if let Some(customer) = payload.customer {
                return Ok(customer.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No customer returned from update".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Delete a customer.
    ///
    /// Note: Customers with orders cannot be deleted.
    ///
    /// # Arguments
    ///
    /// * `id` - Customer ID to delete
    ///
    /// # Returns
    ///
    /// Returns the deleted customer's ID on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails, the customer has orders,
    /// or returns user errors.
    #[instrument(skip(self), fields(customer_id = %id))]
    pub async fn delete_customer(&self, id: &str) -> Result<String, AdminShopifyError> {
        use queries::customer_delete::{CustomerDeleteInput, Variables};

        let variables = Variables {
            input: CustomerDeleteInput { id: id.to_string() },
        };

        let response: <CustomerDelete as GraphQLQuery>::ResponseData =
            self.execute::<CustomerDelete>(variables).await?;

        if let Some(payload) = response.customer_delete {
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

            if let Some(deleted_id) = payload.deleted_customer_id {
                return Ok(deleted_id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Customer deletion failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Add tags to a customer.
    ///
    /// # Arguments
    ///
    /// * `id` - Customer ID
    /// * `tags` - Tags to add
    ///
    /// # Returns
    ///
    /// Returns the updated tags list on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %id))]
    pub async fn add_customer_tags(
        &self,
        id: &str,
        tags: Vec<String>,
    ) -> Result<Vec<String>, AdminShopifyError> {
        use queries::tags_add::Variables;

        let variables = Variables {
            id: id.to_string(),
            tags,
        };

        let response: <TagsAdd as GraphQLQuery>::ResponseData =
            self.execute::<TagsAdd>(variables).await?;

        if let Some(payload) = response.tags_add {
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

            // Return empty tags if node is not available
            return Ok(vec![]);
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Tags add failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Remove tags from a customer.
    ///
    /// # Arguments
    ///
    /// * `id` - Customer ID
    /// * `tags` - Tags to remove
    ///
    /// # Returns
    ///
    /// Returns the updated tags list on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %id))]
    pub async fn remove_customer_tags(
        &self,
        id: &str,
        tags: Vec<String>,
    ) -> Result<Vec<String>, AdminShopifyError> {
        use queries::tags_remove::Variables;

        let variables = Variables {
            id: id.to_string(),
            tags,
        };

        let response: <TagsRemove as GraphQLQuery>::ResponseData =
            self.execute::<TagsRemove>(variables).await?;

        if let Some(payload) = response.tags_remove {
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

            return Ok(vec![]);
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Tags remove failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Send account invitation email to a customer.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Customer ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %customer_id))]
    pub async fn send_customer_invite(&self, customer_id: &str) -> Result<(), AdminShopifyError> {
        use queries::customer_send_account_invite_email::Variables;

        let variables = Variables {
            customer_id: customer_id.to_string(),
        };

        let response: <CustomerSendAccountInviteEmail as GraphQLQuery>::ResponseData = self
            .execute::<CustomerSendAccountInviteEmail>(variables)
            .await?;

        if let Some(payload) = response.customer_send_account_invite_email {
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
            message: "Send invite failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Generate account activation URL for a customer.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Customer ID
    ///
    /// # Returns
    ///
    /// Returns the activation URL string.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %customer_id))]
    pub async fn generate_customer_activation_url(
        &self,
        customer_id: &str,
    ) -> Result<String, AdminShopifyError> {
        use queries::customer_generate_account_activation_url::Variables;

        let variables = Variables {
            customer_id: customer_id.to_string(),
        };

        let response: <CustomerGenerateAccountActivationUrl as GraphQLQuery>::ResponseData = self
            .execute::<CustomerGenerateAccountActivationUrl>(variables)
            .await?;

        if let Some(payload) = response.customer_generate_account_activation_url {
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

            if let Some(url) = payload.account_activation_url {
                return Ok(url);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Generate activation URL failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Create a new address for a customer.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Shopify customer GID
    /// * `address` - Address details
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, address), fields(customer_id = %customer_id))]
    pub async fn create_customer_address(
        &self,
        customer_id: &str,
        address: crate::shopify::types::AddressInput,
    ) -> Result<crate::shopify::types::Address, AdminShopifyError> {
        use queries::customer_address_create::{CountryCode, MailingAddressInput, Variables};

        // Convert country code string to enum
        let country_code = address.country_code.and_then(|code| {
            // Try to parse common country codes
            match code.to_uppercase().as_str() {
                "US" => Some(CountryCode::US),
                "CA" => Some(CountryCode::CA),
                "GB" => Some(CountryCode::GB),
                "AU" => Some(CountryCode::AU),
                "DE" => Some(CountryCode::DE),
                "FR" => Some(CountryCode::FR),
                "JP" => Some(CountryCode::JP),
                "CN" => Some(CountryCode::CN),
                "MX" => Some(CountryCode::MX),
                "BR" => Some(CountryCode::BR),
                _ => None,
            }
        });

        let variables = Variables {
            customer_id: customer_id.to_string(),
            address: MailingAddressInput {
                address1: address.address1,
                address2: address.address2,
                city: address.city,
                province_code: address.province_code,
                country_code,
                zip: address.zip,
                first_name: address.first_name,
                last_name: address.last_name,
                company: address.company,
                phone: address.phone,
            },
        };

        let response = self.execute::<CustomerAddressCreate>(variables).await?;

        if let Some(payload) = response.customer_address_create {
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

            if let Some(addr) = payload.address {
                return Ok(crate::shopify::types::Address {
                    id: Some(addr.id),
                    address1: addr.address1,
                    address2: addr.address2,
                    city: addr.city,
                    province_code: addr.province_code,
                    country_code: addr.country_code_v2.map(|c| format!("{c:?}")),
                    zip: addr.zip,
                    first_name: addr.first_name,
                    last_name: addr.last_name,
                    company: addr.company,
                    phone: addr.phone,
                });
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Create customer address failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update an existing customer address.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Shopify customer GID
    /// * `address_id` - Shopify address GID
    /// * `address` - Updated address details
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, address), fields(customer_id = %customer_id, address_id = %address_id))]
    pub async fn update_customer_address(
        &self,
        customer_id: &str,
        address_id: &str,
        address: crate::shopify::types::AddressInput,
    ) -> Result<crate::shopify::types::Address, AdminShopifyError> {
        use queries::customer_address_update::{CountryCode, MailingAddressInput, Variables};

        // Convert country code string to enum
        let country_code =
            address
                .country_code
                .and_then(|code| match code.to_uppercase().as_str() {
                    "US" => Some(CountryCode::US),
                    "CA" => Some(CountryCode::CA),
                    "GB" => Some(CountryCode::GB),
                    "AU" => Some(CountryCode::AU),
                    "DE" => Some(CountryCode::DE),
                    "FR" => Some(CountryCode::FR),
                    "JP" => Some(CountryCode::JP),
                    "CN" => Some(CountryCode::CN),
                    "MX" => Some(CountryCode::MX),
                    "BR" => Some(CountryCode::BR),
                    _ => None,
                });

        let variables = Variables {
            customer_id: customer_id.to_string(),
            address_id: address_id.to_string(),
            address: MailingAddressInput {
                address1: address.address1,
                address2: address.address2,
                city: address.city,
                province_code: address.province_code,
                country_code,
                zip: address.zip,
                first_name: address.first_name,
                last_name: address.last_name,
                company: address.company,
                phone: address.phone,
            },
        };

        let response = self.execute::<CustomerAddressUpdate>(variables).await?;

        if let Some(payload) = response.customer_address_update {
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

            if let Some(addr) = payload.address {
                return Ok(crate::shopify::types::Address {
                    id: Some(addr.id),
                    address1: addr.address1,
                    address2: addr.address2,
                    city: addr.city,
                    province_code: addr.province_code,
                    country_code: addr.country_code_v2.map(|c| format!("{c:?}")),
                    zip: addr.zip,
                    first_name: addr.first_name,
                    last_name: addr.last_name,
                    company: addr.company,
                    phone: addr.phone,
                });
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Update customer address failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Delete a customer address.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Shopify customer GID
    /// * `address_id` - Shopify address GID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %customer_id, address_id = %address_id))]
    pub async fn delete_customer_address(
        &self,
        customer_id: &str,
        address_id: &str,
    ) -> Result<String, AdminShopifyError> {
        use queries::customer_address_delete::Variables;

        let variables = Variables {
            customer_id: customer_id.to_string(),
            address_id: address_id.to_string(),
        };

        let response: <CustomerAddressDelete as GraphQLQuery>::ResponseData =
            self.execute::<CustomerAddressDelete>(variables).await?;

        if let Some(payload) = response.customer_address_delete {
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

            if let Some(deleted_id) = payload.deleted_address_id {
                return Ok(deleted_id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Delete customer address failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Set a customer's default address.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Shopify customer GID
    /// * `address_id` - Shopify address GID to set as default
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %customer_id, address_id = %address_id))]
    pub async fn set_customer_default_address(
        &self,
        customer_id: &str,
        address_id: &str,
    ) -> Result<(), AdminShopifyError> {
        use queries::customer_update_default_address::Variables;

        let variables = Variables {
            customer_id: customer_id.to_string(),
            address_id: address_id.to_string(),
        };

        let response: <CustomerUpdateDefaultAddress as GraphQLQuery>::ResponseData = self
            .execute::<CustomerUpdateDefaultAddress>(variables)
            .await?;

        if let Some(payload) = response.customer_update_default_address {
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
            message: "Set default address failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update customer email marketing consent.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Shopify customer GID
    /// * `marketing_state` - New marketing state (SUBSCRIBED, UNSUBSCRIBED, etc.)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %customer_id))]
    pub async fn update_customer_email_marketing(
        &self,
        customer_id: &str,
        marketing_state: &str,
    ) -> Result<(), AdminShopifyError> {
        use queries::customer_email_marketing_consent_update::{
            CustomerEmailMarketingConsentInput, CustomerEmailMarketingConsentUpdateInput,
            CustomerEmailMarketingState, Variables,
        };

        let state = match marketing_state {
            "SUBSCRIBED" => CustomerEmailMarketingState::SUBSCRIBED,
            "UNSUBSCRIBED" => CustomerEmailMarketingState::UNSUBSCRIBED,
            "PENDING" => CustomerEmailMarketingState::PENDING,
            // Handles "NOT_SUBSCRIBED" and any unknown values
            _ => CustomerEmailMarketingState::NOT_SUBSCRIBED,
        };

        let variables = Variables {
            input: CustomerEmailMarketingConsentUpdateInput {
                customer_id: customer_id.to_string(),
                email_marketing_consent: CustomerEmailMarketingConsentInput {
                    consent_updated_at: None,
                    marketing_opt_in_level: None,
                    marketing_state: state,
                    source_location_id: None,
                },
            },
        };

        let response: <CustomerEmailMarketingConsentUpdate as GraphQLQuery>::ResponseData = self
            .execute::<CustomerEmailMarketingConsentUpdate>(variables)
            .await?;

        if let Some(payload) = response.customer_email_marketing_consent_update {
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
            message: "Update email marketing consent failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update customer SMS marketing consent.
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Shopify customer GID
    /// * `marketing_state` - New marketing state (SUBSCRIBED, UNSUBSCRIBED, etc.)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_id = %customer_id))]
    pub async fn update_customer_sms_marketing(
        &self,
        customer_id: &str,
        marketing_state: &str,
    ) -> Result<(), AdminShopifyError> {
        use queries::customer_sms_marketing_consent_update::{
            CustomerSmsMarketingConsentInput, CustomerSmsMarketingConsentUpdateInput,
            CustomerSmsMarketingState, Variables,
        };

        let state = match marketing_state {
            "SUBSCRIBED" => CustomerSmsMarketingState::SUBSCRIBED,
            "UNSUBSCRIBED" => CustomerSmsMarketingState::UNSUBSCRIBED,
            "PENDING" => CustomerSmsMarketingState::PENDING,
            // Handles "NOT_SUBSCRIBED" and any unknown values
            _ => CustomerSmsMarketingState::NOT_SUBSCRIBED,
        };

        let variables = Variables {
            input: CustomerSmsMarketingConsentUpdateInput {
                customer_id: customer_id.to_string(),
                sms_marketing_consent: CustomerSmsMarketingConsentInput {
                    consent_updated_at: None,
                    marketing_opt_in_level: None,
                    marketing_state: state,
                    source_location_id: None,
                },
            },
        };

        let response: <CustomerSmsMarketingConsentUpdate as GraphQLQuery>::ResponseData = self
            .execute::<CustomerSmsMarketingConsentUpdate>(variables)
            .await?;

        if let Some(payload) = response.customer_sms_marketing_consent_update {
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
            message: "Update SMS marketing consent failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Merge two customers.
    ///
    /// # Arguments
    ///
    /// * `customer_one_id` - Customer to merge INTO (will remain)
    /// * `customer_two_id` - Customer to merge FROM (will be deleted)
    /// * `overrides` - Fields to take from `customer_two` instead of `customer_one`
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(customer_one_id = %customer_one_id, customer_two_id = %customer_two_id))]
    pub async fn merge_customers(
        &self,
        customer_one_id: &str,
        customer_two_id: &str,
        overrides: super::types::CustomerMergeOverrides,
    ) -> Result<String, AdminShopifyError> {
        use queries::customer_merge::{CustomerMergeOverrideFields, Variables};

        let has_overrides = overrides.first_name
            || overrides.last_name
            || overrides.email
            || overrides.phone
            || overrides.default_address;

        let override_fields = if has_overrides {
            Some(CustomerMergeOverrideFields {
                customer_id_of_first_name_to_keep: if overrides.first_name {
                    Some(customer_two_id.to_string())
                } else {
                    None
                },
                customer_id_of_last_name_to_keep: if overrides.last_name {
                    Some(customer_two_id.to_string())
                } else {
                    None
                },
                customer_id_of_email_to_keep: if overrides.email {
                    Some(customer_two_id.to_string())
                } else {
                    None
                },
                customer_id_of_phone_number_to_keep: if overrides.phone {
                    Some(customer_two_id.to_string())
                } else {
                    None
                },
                customer_id_of_default_address_to_keep: if overrides.default_address {
                    Some(customer_two_id.to_string())
                } else {
                    None
                },
                note: None,
                tags: None,
            })
        } else {
            None
        };

        let variables = Variables {
            customer_one_id: customer_one_id.to_string(),
            customer_two_id: customer_two_id.to_string(),
            override_fields,
        };

        let response: <CustomerMerge as GraphQLQuery>::ResponseData =
            self.execute::<CustomerMerge>(variables).await?;

        if let Some(payload) = response.customer_merge {
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

            if let Some(resulting_id) = payload.resulting_customer_id {
                return Ok(resulting_id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Merge customers failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
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

    /// Get inventory items with pagination.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of items to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_inventory_items(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
    ) -> Result<InventoryItemConnection, AdminShopifyError> {
        let variables = queries::get_inventory_items::Variables {
            first: Some(first),
            after,
            query,
        };

        let response = self.execute::<GetInventoryItems>(variables).await?;

        Ok(convert_inventory_item_connection(response))
    }

    /// Get a single inventory item by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify inventory item ID (e.g., `gid://shopify/InventoryItem/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or the item is not found.
    #[instrument(skip(self), fields(id = %id))]
    pub async fn get_inventory_item(&self, id: &str) -> Result<InventoryItem, AdminShopifyError> {
        let variables = queries::get_inventory_item::Variables { id: id.to_string() };

        let response = self.execute::<GetInventoryItem>(variables).await?;

        response
            .inventory_item
            .map(convert_single_inventory_item)
            .ok_or_else(|| AdminShopifyError::NotFound(format!("Inventory item {id} not found")))
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

    /// Update inventory item properties.
    ///
    /// # Arguments
    ///
    /// * `id` - Inventory item ID
    /// * `input` - Fields to update
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input), fields(id = %id))]
    pub async fn update_inventory_item(
        &self,
        id: &str,
        input: &super::types::InventoryItemUpdateInput,
    ) -> Result<InventoryItem, AdminShopifyError> {
        use queries::update_inventory_item::{CountryCode, InventoryItemInput};

        // Convert country code string to enum
        let country_code = input
            .country_code_of_origin
            .as_ref()
            .map(|code| match code.as_str() {
                "US" => CountryCode::US,
                "CN" => CountryCode::CN,
                "VN" => CountryCode::VN,
                "BD" => CountryCode::BD,
                "IN" => CountryCode::IN,
                "ID" => CountryCode::ID,
                "TH" => CountryCode::TH,
                "PK" => CountryCode::PK,
                "TR" => CountryCode::TR,
                "KH" => CountryCode::KH,
                "MX" => CountryCode::MX,
                "IT" => CountryCode::IT,
                "PT" => CountryCode::PT,
                "ES" => CountryCode::ES,
                "GB" => CountryCode::GB,
                "CA" => CountryCode::CA,
                "AU" => CountryCode::AU,
                "JP" => CountryCode::JP,
                "KR" => CountryCode::KR,
                "TW" => CountryCode::TW,
                _ => CountryCode::Other(code.clone()),
            });

        let variables = queries::update_inventory_item::Variables {
            id: id.to_string(),
            input: InventoryItemInput {
                tracked: input.tracked,
                country_code_of_origin: country_code,
                province_code_of_origin: input.province_code_of_origin.clone(),
                harmonized_system_code: input.harmonized_system_code.clone(),
                cost: None,
                country_harmonized_system_codes: None,
                measurement: None,
                requires_shipping: input.requires_shipping,
                sku: None, // SKU updates require variant mutation
            },
        };

        let response = self.execute::<UpdateInventoryItem>(variables).await?;

        // Check for user errors
        if let Some(ref payload) = response.inventory_item_update
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| e.message.clone())
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        // Re-fetch the item to get the full updated data
        self.get_inventory_item(id).await
    }

    /// Move inventory from one location to another.
    ///
    /// # Arguments
    ///
    /// * `inventory_item_id` - The inventory item ID
    /// * `from_location_id` - Source location ID
    /// * `to_location_id` - Destination location ID
    /// * `quantity` - Quantity to move
    /// * `reason` - Reason for the move
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn move_inventory(
        &self,
        inventory_item_id: &str,
        from_location_id: &str,
        to_location_id: &str,
        quantity: i64,
        reason: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        use queries::move_inventory::{
            InventoryMoveQuantitiesInput, InventoryMoveQuantityChange,
            InventoryMoveQuantityTerminalInput,
        };

        let variables = queries::move_inventory::Variables {
            input: InventoryMoveQuantitiesInput {
                changes: vec![InventoryMoveQuantityChange {
                    inventory_item_id: inventory_item_id.to_string(),
                    quantity,
                    from: InventoryMoveQuantityTerminalInput {
                        location_id: from_location_id.to_string(),
                        name: "available".to_string(),
                        ledger_document_uri: None,
                        change_from_quantity: None,
                    },
                    to: InventoryMoveQuantityTerminalInput {
                        location_id: to_location_id.to_string(),
                        name: "available".to_string(),
                        ledger_document_uri: None,
                        change_from_quantity: None,
                    },
                }],
                reason: reason.unwrap_or("Stock transfer").to_string(),
                reference_document_uri: String::new(),
            },
        };

        let response = self.execute::<MoveInventory>(variables).await?;

        // Check for user errors
        if let Some(ref payload) = response.inventory_move_quantities
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

    /// Activate inventory tracking at a location.
    ///
    /// # Arguments
    ///
    /// * `inventory_item_id` - The inventory item ID
    /// * `location_id` - The location to activate at
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn activate_inventory(
        &self,
        inventory_item_id: &str,
        location_id: &str,
    ) -> Result<(), AdminShopifyError> {
        let variables = queries::activate_inventory::Variables {
            inventory_item_id: inventory_item_id.to_string(),
            location_id: location_id.to_string(),
        };

        let response = self.execute::<ActivateInventory>(variables).await?;

        // Check for user errors
        if let Some(ref payload) = response.inventory_activate
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

    /// Deactivate inventory tracking at a location.
    ///
    /// # Arguments
    ///
    /// * `inventory_level_id` - The inventory level ID (not item ID)
    /// * `location_id` - The location to deactivate at
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn deactivate_inventory(
        &self,
        inventory_level_id: &str,
    ) -> Result<(), AdminShopifyError> {
        let variables = queries::deactivate_inventory::Variables {
            inventory_level_id: inventory_level_id.to_string(),
        };

        let response = self.execute::<DeactivateInventory>(variables).await?;

        // Check for user errors
        if let Some(ref payload) = response.inventory_deactivate
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

    /// Get a paginated list of gift cards with optional sorting.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of gift cards to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query (Shopify query syntax)
    /// * `sort_key` - Optional sort key
    /// * `reverse` - Whether to reverse sort order
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
        sort_key: Option<GiftCardSortKey>,
        reverse: bool,
    ) -> Result<GiftCardConnection, AdminShopifyError> {
        use queries::get_gift_cards::{GiftCardSortKeys, Variables};

        let sort_key_gql = sort_key.map(|sk| match sk {
            GiftCardSortKey::AmountSpent => GiftCardSortKeys::AMOUNT_SPENT,
            GiftCardSortKey::Balance => GiftCardSortKeys::BALANCE,
            GiftCardSortKey::Code => GiftCardSortKeys::CODE,
            GiftCardSortKey::CreatedAt => GiftCardSortKeys::CREATED_AT,
            GiftCardSortKey::CustomerName => GiftCardSortKeys::CUSTOMER_NAME,
            GiftCardSortKey::DisabledAt => GiftCardSortKeys::DISABLED_AT,
            GiftCardSortKey::ExpiresOn => GiftCardSortKeys::EXPIRES_ON,
            GiftCardSortKey::Id => GiftCardSortKeys::ID,
            GiftCardSortKey::InitialValue => GiftCardSortKeys::INITIAL_VALUE,
            GiftCardSortKey::UpdatedAt => GiftCardSortKeys::UPDATED_AT,
        });

        let variables = Variables {
            first: Some(first),
            after,
            query,
            sort_key: sort_key_gql,
            reverse: Some(reverse),
        };

        let response = self.execute::<GetGiftCards>(variables).await?;

        let gift_cards: Vec<GiftCard> = response
            .gift_cards
            .edges
            .into_iter()
            .map(|e| {
                let gc = e.node;
                GiftCard {
                    id: gc.id,
                    last_characters: gc.last_characters,
                    masked_code: Some(gc.masked_code),
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
                    deactivated_at: gc.deactivated_at,
                    created_at: gc.created_at,
                    updated_at: Some(gc.updated_at),
                    customer_id: gc.customer.as_ref().map(|c| c.id.clone()),
                    #[allow(deprecated)]
                    customer_email: gc.customer.as_ref().and_then(|c| c.email.clone()),
                    customer_name: gc.customer.as_ref().map(|c| c.display_name.clone()),
                    note: gc.note,
                    order_id: gc.order.as_ref().map(|o| o.id.clone()),
                    order_name: gc.order.as_ref().map(|o| o.name.clone()),
                }
            })
            .collect();

        Ok(GiftCardConnection {
            gift_cards,
            page_info: PageInfo {
                has_next_page: response.gift_cards.page_info.has_next_page,
                has_previous_page: response.gift_cards.page_info.has_previous_page,
                start_cursor: response.gift_cards.page_info.start_cursor,
                end_cursor: response.gift_cards.page_info.end_cursor,
            },
            total_count: None,
        })
    }

    /// Get the count of gift cards matching a query.
    ///
    /// # Arguments
    ///
    /// * `query` - Optional search query (Shopify query syntax)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_gift_cards_count(
        &self,
        query: Option<String>,
    ) -> Result<i64, AdminShopifyError> {
        let variables = queries::get_gift_cards_count::Variables { query };
        let response = self.execute::<GetGiftCardsCount>(variables).await?;
        Ok(response.gift_cards_count.map_or(0, |c| c.count))
    }

    /// Get a single gift card with full details including transactions.
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or gift card not found.
    #[instrument(skip(self))]
    pub async fn get_gift_card_detail(
        &self,
        id: &str,
    ) -> Result<GiftCardDetail, AdminShopifyError> {
        let variables = queries::get_gift_card_detail::Variables { id: id.to_string() };
        let response = self.execute::<GetGiftCardDetail>(variables).await?;

        let gc = response.gift_card.ok_or_else(|| {
            AdminShopifyError::GraphQL(vec![GraphQLError {
                message: "Gift card not found".to_string(),
                locations: vec![],
                path: vec![],
            }])
        })?;

        use queries::get_gift_card_detail::GetGiftCardDetailGiftCardTransactionsEdgesNodeOn;

        let transactions: Vec<GiftCardTransaction> = gc
            .transactions
            .map(|t| {
                t.edges
                    .into_iter()
                    .map(|e| {
                        let tx = e.node;
                        let is_credit = matches!(
                            tx.on,
                            GetGiftCardDetailGiftCardTransactionsEdgesNodeOn::GiftCardCreditTransaction
                        );
                        GiftCardTransaction {
                            id: tx.id,
                            amount: Money {
                                amount: tx.amount.amount,
                                currency_code: format!("{:?}", tx.amount.currency_code),
                            },
                            processed_at: tx.processed_at,
                            note: tx.note,
                            is_credit,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        #[allow(deprecated)]
        let recipient = gc.recipient_attributes.map(|r| GiftCardRecipient {
            recipient_id: Some(r.recipient.id.clone()),
            recipient_name: Some(r.recipient.display_name.clone()),
            recipient_email: r.recipient.email.clone(),
            preferred_name: r.preferred_name,
            message: r.message,
            send_notification_at: r.send_notification_at,
        });

        Ok(GiftCardDetail {
            id: gc.id,
            last_characters: gc.last_characters,
            masked_code: gc.masked_code,
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
            deactivated_at: gc.deactivated_at,
            created_at: gc.created_at,
            updated_at: gc.updated_at,
            note: gc.note,
            template_suffix: gc.template_suffix,
            customer_id: gc.customer.as_ref().map(|c| c.id.clone()),
            customer_name: gc.customer.as_ref().map(|c| c.display_name.clone()),
            #[allow(deprecated)]
            customer_email: gc.customer.as_ref().and_then(|c| c.email.clone()),
            #[allow(deprecated)]
            customer_phone: gc.customer.as_ref().and_then(|c| c.phone.clone()),
            recipient,
            order_id: gc.order.as_ref().map(|o| o.id.clone()),
            order_name: gc.order.as_ref().map(|o| o.name.clone()),
            order_created_at: gc.order.as_ref().map(|o| o.created_at.clone()),
            transactions,
        })
    }

    /// Get gift card configuration (shop limits).
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_gift_card_configuration(
        &self,
    ) -> Result<GiftCardConfiguration, AdminShopifyError> {
        let variables = queries::get_gift_card_configuration::Variables {};
        let response = self.execute::<GetGiftCardConfiguration>(variables).await?;
        let config = response.gift_card_configuration;

        Ok(GiftCardConfiguration {
            issue_limit: Some(Money {
                amount: config.issue_limit.amount,
                currency_code: format!("{:?}", config.issue_limit.currency_code),
            }),
            purchase_limit: Some(Money {
                amount: config.purchase_limit.amount,
                currency_code: format!("{:?}", config.purchase_limit.currency_code),
            }),
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
        recipient_id: Option<&str>,
        recipient_message: Option<&str>,
    ) -> Result<(String, String), AdminShopifyError> {
        use queries::gift_card_create::{GiftCardCreateInput, GiftCardRecipientInput, Variables};

        let recipient_attributes = recipient_id.map(|id| GiftCardRecipientInput {
            id: id.to_string(),
            message: recipient_message.map(String::from),
            preferred_name: None,
            send_notification_at: None,
        });

        let variables = Variables {
            input: GiftCardCreateInput {
                initial_value: initial_value.to_string(),
                customer_id: customer_id.map(String::from),
                expires_on: expires_on.map(String::from),
                note: note.map(String::from),
                code: None,
                template_suffix: None,
                recipient_attributes,
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

    /// Deactivate a gift card permanently.
    ///
    /// Warning: This action cannot be undone. Once deactivated, the gift card
    /// can no longer be used.
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID to deactivate
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn deactivate_gift_card(&self, id: &str) -> Result<(), AdminShopifyError> {
        let variables = queries::gift_card_deactivate::Variables { id: id.to_string() };

        let response = self.execute::<GiftCardDeactivate>(variables).await?;

        if let Some(payload) = response.gift_card_deactivate
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

    /// Update a gift card's details.
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID to update
    /// * `note` - New internal note (None = no change)
    /// * `expires_on` - New expiration date (None = no change, Some("") = remove expiration)
    /// * `customer_id` - Customer to assign (None = no change)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn update_gift_card(
        &self,
        id: &str,
        note: Option<&str>,
        expires_on: Option<&str>,
        customer_id: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        use queries::gift_card_update::{GiftCardUpdateInput, Variables};

        let variables = Variables {
            id: id.to_string(),
            input: GiftCardUpdateInput {
                note: note.map(String::from),
                expires_on: expires_on.map(String::from),
                customer_id: customer_id.map(String::from),
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

    /// Credit a gift card (add funds).
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID
    /// * `amount` - Amount to credit as decimal string
    /// * `currency_code` - Currency code (e.g., "USD")
    /// * `note` - Optional note for the transaction
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn credit_gift_card(
        &self,
        id: &str,
        amount: &str,
        currency_code: &str,
        note: Option<&str>,
    ) -> Result<GiftCardTransaction, AdminShopifyError> {
        use queries::gift_card_credit::{CurrencyCode, GiftCardCreditInput, MoneyInput, Variables};

        // Map common currency codes, default to USD
        let currency = match currency_code.to_uppercase().as_str() {
            "CAD" => CurrencyCode::CAD,
            "EUR" => CurrencyCode::EUR,
            "GBP" => CurrencyCode::GBP,
            "AUD" => CurrencyCode::AUD,
            _ => CurrencyCode::USD,
        };

        let variables = Variables {
            id: id.to_string(),
            credit_input: GiftCardCreditInput {
                credit_amount: MoneyInput {
                    amount: amount.to_string(),
                    currency_code: currency,
                },
                note: note.map(String::from),
                processed_at: None,
            },
        };

        let response = self.execute::<GiftCardCredit>(variables).await?;

        if let Some(payload) = response.gift_card_credit {
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

            if let Some(tx) = payload.gift_card_credit_transaction {
                return Ok(GiftCardTransaction {
                    id: tx.id,
                    amount: Money {
                        amount: tx.amount.amount,
                        currency_code: format!("{:?}", tx.amount.currency_code),
                    },
                    processed_at: tx.processed_at,
                    note: tx.note,
                    is_credit: true,
                });
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No transaction returned from credit".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Debit a gift card (remove funds).
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID
    /// * `amount` - Amount to debit as decimal string
    /// * `currency_code` - Currency code (e.g., "USD")
    /// * `note` - Optional note for the transaction
    ///
    /// # Errors
    ///
    /// Returns an error if insufficient funds or API request fails.
    #[instrument(skip(self))]
    pub async fn debit_gift_card(
        &self,
        id: &str,
        amount: &str,
        currency_code: &str,
        note: Option<&str>,
    ) -> Result<GiftCardTransaction, AdminShopifyError> {
        use queries::gift_card_debit::{CurrencyCode, GiftCardDebitInput, MoneyInput, Variables};

        // Map common currency codes, default to USD
        let currency = match currency_code.to_uppercase().as_str() {
            "CAD" => CurrencyCode::CAD,
            "EUR" => CurrencyCode::EUR,
            "GBP" => CurrencyCode::GBP,
            "AUD" => CurrencyCode::AUD,
            _ => CurrencyCode::USD,
        };

        let variables = Variables {
            id: id.to_string(),
            debit_input: GiftCardDebitInput {
                debit_amount: MoneyInput {
                    amount: amount.to_string(),
                    currency_code: currency,
                },
                note: note.map(String::from),
                processed_at: None,
            },
        };

        let response = self.execute::<GiftCardDebit>(variables).await?;

        if let Some(payload) = response.gift_card_debit {
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

            if let Some(tx) = payload.gift_card_debit_transaction {
                return Ok(GiftCardTransaction {
                    id: tx.id,
                    amount: Money {
                        amount: tx.amount.amount,
                        currency_code: format!("{:?}", tx.amount.currency_code),
                    },
                    processed_at: tx.processed_at,
                    note: tx.note,
                    is_credit: false,
                });
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No transaction returned from debit".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Send gift card notification to the assigned customer.
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID
    ///
    /// # Errors
    ///
    /// Returns an error if no customer is assigned or API request fails.
    #[instrument(skip(self))]
    pub async fn send_gift_card_notification_to_customer(
        &self,
        id: &str,
    ) -> Result<(), AdminShopifyError> {
        let variables =
            queries::gift_card_send_notification_to_customer::Variables { id: id.to_string() };

        let response = self
            .execute::<GiftCardSendNotificationToCustomer>(variables)
            .await?;

        if let Some(payload) = response.gift_card_send_notification_to_customer
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

    /// Send gift card notification to the designated recipient.
    ///
    /// # Arguments
    ///
    /// * `id` - Gift card ID
    ///
    /// # Errors
    ///
    /// Returns an error if no recipient is set or API request fails.
    #[instrument(skip(self))]
    pub async fn send_gift_card_notification_to_recipient(
        &self,
        id: &str,
    ) -> Result<(), AdminShopifyError> {
        let variables =
            queries::gift_card_send_notification_to_recipient::Variables { id: id.to_string() };

        let response = self
            .execute::<GiftCardSendNotificationToRecipient>(variables)
            .await?;

        if let Some(payload) = response.gift_card_send_notification_to_recipient
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

    /// Get a paginated list of discounts with sorting/filtering (all types).
    ///
    /// # Arguments
    ///
    /// * `first` - Number of discounts to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query
    /// * `sort_key` - Sort key
    /// * `reverse` - Reverse sort order
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_discounts_for_list(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
        sort_key: Option<DiscountSortKey>,
        reverse: bool,
    ) -> Result<DiscountListConnection, AdminShopifyError> {
        use queries::get_discount_nodes::{DiscountSortKeys, Variables};

        let gql_sort_key = sort_key.map(|sk| match sk {
            DiscountSortKey::Title => DiscountSortKeys::TITLE,
            DiscountSortKey::CreatedAt => DiscountSortKeys::CREATED_AT,
            DiscountSortKey::UpdatedAt => DiscountSortKeys::UPDATED_AT,
            DiscountSortKey::StartsAt => DiscountSortKeys::STARTS_AT,
            DiscountSortKey::EndsAt => DiscountSortKeys::ENDS_AT,
            DiscountSortKey::Id => DiscountSortKeys::ID,
        });

        let variables = Variables {
            first: Some(first),
            after,
            query,
            sort_key: gql_sort_key,
            reverse: Some(reverse),
        };

        let response = self.execute::<GetDiscountNodes>(variables).await?;

        let discounts = Self::convert_discount_nodes_to_list(response.discount_nodes);

        Ok(discounts)
    }

    /// Convert GraphQL discount nodes to list items.
    fn convert_discount_nodes_to_list(
        nodes: queries::get_discount_nodes::GetDiscountNodesDiscountNodes,
    ) -> DiscountListConnection {
        use queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscount as Discount;

        let discounts: Vec<DiscountListItem> = nodes
            .edges
            .into_iter()
            .filter_map(|edge| {
                let id = edge.node.id;
                Self::convert_single_discount_node(id, edge.node.discount)
            })
            .collect();

        DiscountListConnection {
            discounts,
            page_info: PageInfo {
                has_next_page: nodes.page_info.has_next_page,
                has_previous_page: nodes.page_info.has_previous_page,
                start_cursor: nodes.page_info.start_cursor,
                end_cursor: nodes.page_info.end_cursor,
            },
        }
    }

    /// Convert a single discount node to a list item.
    fn convert_single_discount_node(
        id: String,
        discount: queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscount,
    ) -> Option<DiscountListItem> {
        use queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscount as Discount;

        match discount {
            Discount::DiscountCodeBasic(basic) => Some(Self::convert_code_basic_node(id, basic)),
            Discount::DiscountCodeBxgy(bxgy) => Some(Self::convert_code_bxgy_node(id, bxgy)),
            Discount::DiscountCodeFreeShipping(fs) => {
                Some(Self::convert_code_freeshipping_node(id, fs))
            }
            Discount::DiscountAutomaticBasic(auto) => Some(Self::convert_auto_basic_node(id, auto)),
            Discount::DiscountAutomaticBxgy(auto) => Some(Self::convert_auto_bxgy_node(id, auto)),
            Discount::DiscountAutomaticFreeShipping(auto) => {
                Some(Self::convert_auto_freeshipping_node(id, auto))
            }
            _ => None,
        }
    }

    /// Convert code basic discount to list item.
    fn convert_code_basic_node(
        id: String,
        basic: queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBasic,
    ) -> DiscountListItem {
        let code = basic.codes.edges.first().map(|e| e.node.code.clone());
        let value = Self::convert_discount_value_nodes(&basic.customer_gets.value);
        let minimum = Self::convert_minimum_requirement_nodes(basic.minimum_requirement.as_ref());
        let combines_with = Self::convert_combines_with_nodes(&basic.combines_with);
        let code_count = i64::try_from(basic.codes.edges.len()).unwrap_or(0);

        DiscountListItem {
            id,
            title: basic.title,
            code,
            code_count,
            method: DiscountMethod::Code,
            discount_type: DiscountType::Basic,
            status: Self::convert_status_nodes(&basic.status),
            value,
            starts_at: Some(basic.starts_at),
            ends_at: basic.ends_at,
            usage_limit: basic.usage_limit,
            usage_count: basic.async_usage_count,
            once_per_customer: basic.applies_once_per_customer,
            combines_with,
            minimum_requirement: minimum,
        }
    }

    /// Convert code BXGY discount to list item.
    fn convert_code_bxgy_node(
        id: String,
        bxgy: queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBxgy,
    ) -> DiscountListItem {
        let code = bxgy.codes.edges.first().map(|e| e.node.code.clone());
        let combines_with = Self::convert_combines_with_nodes(&bxgy.combines_with);
        let code_count = i64::try_from(bxgy.codes.edges.len()).unwrap_or(0);

        DiscountListItem {
            id,
            title: bxgy.title,
            code,
            code_count,
            method: DiscountMethod::Code,
            discount_type: DiscountType::BuyXGetY,
            status: Self::convert_status_nodes(&bxgy.status),
            value: None,
            starts_at: Some(bxgy.starts_at),
            ends_at: bxgy.ends_at,
            usage_limit: bxgy.usage_limit,
            usage_count: bxgy.async_usage_count,
            once_per_customer: false,
            combines_with,
            minimum_requirement: DiscountMinimumRequirement::None,
        }
    }

    /// Convert code free shipping discount to list item.
    fn convert_code_freeshipping_node(
        id: String,
        fs: queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeFreeShipping,
    ) -> DiscountListItem {
        let code = fs.codes.edges.first().map(|e| e.node.code.clone());
        let minimum = Self::convert_minimum_requirement_nodes(fs.minimum_requirement.as_ref());
        let combines_with = Self::convert_combines_with_nodes(&fs.combines_with);
        let code_count = i64::try_from(fs.codes.edges.len()).unwrap_or(0);

        DiscountListItem {
            id,
            title: fs.title,
            code,
            code_count,
            method: DiscountMethod::Code,
            discount_type: DiscountType::FreeShipping,
            status: Self::convert_status_nodes(&fs.status),
            value: None,
            starts_at: Some(fs.starts_at),
            ends_at: fs.ends_at,
            usage_limit: fs.usage_limit,
            usage_count: fs.async_usage_count,
            once_per_customer: fs.applies_once_per_customer,
            combines_with,
            minimum_requirement: minimum,
        }
    }

    /// Convert automatic basic discount to list item.
    fn convert_auto_basic_node(
        id: String,
        auto: queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBasic,
    ) -> DiscountListItem {
        let value = Self::convert_discount_value_nodes(&auto.customer_gets.value);
        let minimum = Self::convert_minimum_requirement_nodes(auto.minimum_requirement.as_ref());
        let combines_with = Self::convert_combines_with_nodes(&auto.combines_with);

        DiscountListItem {
            id,
            title: auto.title,
            code: None,
            code_count: 0,
            method: DiscountMethod::Automatic,
            discount_type: DiscountType::Basic,
            status: Self::convert_status_nodes(&auto.status),
            value,
            starts_at: Some(auto.starts_at),
            ends_at: auto.ends_at,
            usage_limit: None,
            usage_count: auto.async_usage_count,
            once_per_customer: false,
            combines_with,
            minimum_requirement: minimum,
        }
    }

    /// Convert automatic BXGY discount to list item.
    fn convert_auto_bxgy_node(
        id: String,
        auto: queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBxgy,
    ) -> DiscountListItem {
        let combines_with = Self::convert_combines_with_nodes(&auto.combines_with);

        DiscountListItem {
            id,
            title: auto.title,
            code: None,
            code_count: 0,
            method: DiscountMethod::Automatic,
            discount_type: DiscountType::BuyXGetY,
            status: Self::convert_status_nodes(&auto.status),
            value: None,
            starts_at: Some(auto.starts_at),
            ends_at: auto.ends_at,
            usage_limit: None,
            usage_count: auto.async_usage_count,
            once_per_customer: false,
            combines_with,
            minimum_requirement: DiscountMinimumRequirement::None,
        }
    }

    /// Convert automatic free shipping discount to list item.
    fn convert_auto_freeshipping_node(
        id: String,
        auto: queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountAutomaticFreeShipping,
    ) -> DiscountListItem {
        let minimum = Self::convert_minimum_requirement_nodes(auto.minimum_requirement.as_ref());
        let combines_with = Self::convert_combines_with_nodes(&auto.combines_with);

        DiscountListItem {
            id,
            title: auto.title,
            code: None,
            code_count: 0,
            method: DiscountMethod::Automatic,
            discount_type: DiscountType::FreeShipping,
            status: Self::convert_status_nodes(&auto.status),
            value: None,
            starts_at: Some(auto.starts_at),
            ends_at: auto.ends_at,
            usage_limit: None,
            usage_count: auto.async_usage_count,
            once_per_customer: false,
            combines_with,
            minimum_requirement: minimum,
        }
    }

    /// Convert discount status from GraphQL nodes query.
    const fn convert_status_nodes(
        status: &queries::get_discount_nodes::DiscountStatus,
    ) -> DiscountStatus {
        match status {
            queries::get_discount_nodes::DiscountStatus::ACTIVE
            | queries::get_discount_nodes::DiscountStatus::Other(_) => DiscountStatus::Active,
            queries::get_discount_nodes::DiscountStatus::EXPIRED => DiscountStatus::Expired,
            queries::get_discount_nodes::DiscountStatus::SCHEDULED => DiscountStatus::Scheduled,
        }
    }

    /// Convert discount value from GraphQL nodes query.
    fn convert_discount_value_nodes(
        value: &queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBasicCustomerGetsValue,
    ) -> Option<DiscountValue> {
        use queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBasicCustomerGetsValue as Value;
        match value {
            Value::DiscountPercentage(p) => Some(DiscountValue::Percentage {
                percentage: p.percentage,
            }),
            Value::DiscountAmount(a) => Some(DiscountValue::FixedAmount {
                amount: a.amount.amount.clone(),
                currency: format!("{:?}", a.amount.currency_code),
            }),
            Value::DiscountOnQuantity(_) => None,
        }
    }

    /// Convert minimum requirement from GraphQL nodes query.
    fn convert_minimum_requirement_nodes(
        req: Option<&queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBasicMinimumRequirement>,
    ) -> DiscountMinimumRequirement {
        use queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBasicMinimumRequirement as Req;
        match req {
            Some(Req::DiscountMinimumQuantity(q)) => DiscountMinimumRequirement::Quantity {
                quantity: q.greater_than_or_equal_to_quantity.clone(),
            },
            Some(Req::DiscountMinimumSubtotal(s)) => DiscountMinimumRequirement::Subtotal {
                amount: s.greater_than_or_equal_to_subtotal.amount.clone(),
                currency: format!("{:?}", s.greater_than_or_equal_to_subtotal.currency_code),
            },
            None => DiscountMinimumRequirement::None,
        }
    }

    /// Convert `combines_with` from GraphQL nodes query.
    const fn convert_combines_with_nodes(
        cw: &queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBasicCombinesWith,
    ) -> DiscountCombinesWith {
        DiscountCombinesWith {
            order_discounts: cw.order_discounts,
            product_discounts: cw.product_discounts,
            shipping_discounts: cw.shipping_discounts,
        }
    }

    /// Activate a discount (code or automatic).
    ///
    /// Detects the discount type and calls the appropriate mutation.
    ///
    /// # Arguments
    ///
    /// * `id` - Discount node ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(discount_id = %id))]
    pub async fn activate_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        // Try code discount activation first
        let code_result = self.activate_code_discount(id).await;
        if code_result.is_ok() {
            return Ok(());
        }

        // If that fails, try automatic discount activation
        self.activate_automatic_discount(id).await
    }

    /// Activate a code discount.
    async fn activate_code_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        let variables = queries::discount_code_activate::Variables { id: id.to_string() };
        let response = self.execute::<DiscountCodeActivate>(variables).await?;

        if let Some(payload) = response.discount_code_activate
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

    /// Activate an automatic discount.
    async fn activate_automatic_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        let variables = queries::discount_automatic_activate::Variables { id: id.to_string() };
        let response = self.execute::<DiscountAutomaticActivate>(variables).await?;

        if let Some(payload) = response.discount_automatic_activate
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

    /// Deactivate an automatic discount.
    ///
    /// # Arguments
    ///
    /// * `id` - Automatic discount node ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(discount_id = %id))]
    pub async fn deactivate_automatic_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        let variables = queries::discount_automatic_deactivate::Variables { id: id.to_string() };
        let response = self
            .execute::<DiscountAutomaticDeactivate>(variables)
            .await?;

        if let Some(payload) = response.discount_automatic_deactivate
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

    /// Delete a discount (code or automatic).
    ///
    /// Detects the discount type and calls the appropriate mutation.
    ///
    /// # Arguments
    ///
    /// * `id` - Discount node ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(discount_id = %id))]
    pub async fn delete_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        // Try code discount deletion first
        let code_result = self.delete_code_discount(id).await;
        if code_result.is_ok() {
            return Ok(());
        }

        // If that fails, try automatic discount deletion
        self.delete_automatic_discount(id).await
    }

    /// Delete a code discount.
    async fn delete_code_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        let variables = queries::discount_code_delete::Variables { id: id.to_string() };
        let response = self.execute::<DiscountCodeDelete>(variables).await?;

        if let Some(payload) = response.discount_code_delete
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

    /// Delete an automatic discount.
    async fn delete_automatic_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        let variables = queries::discount_automatic_delete::Variables { id: id.to_string() };
        let response = self.execute::<DiscountAutomaticDelete>(variables).await?;

        if let Some(payload) = response.discount_automatic_delete
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

    /// Bulk activate code discounts.
    ///
    /// # Arguments
    ///
    /// * `ids` - List of discount node IDs to activate
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn bulk_activate_code_discounts(
        &self,
        ids: Vec<String>,
    ) -> Result<(), AdminShopifyError> {
        let variables = queries::discount_code_bulk_activate::Variables { ids };
        let response = self.execute::<DiscountCodeBulkActivate>(variables).await?;

        if let Some(payload) = response.discount_code_bulk_activate
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

    /// Bulk deactivate code discounts.
    ///
    /// # Arguments
    ///
    /// * `ids` - List of discount node IDs to deactivate
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn bulk_deactivate_code_discounts(
        &self,
        ids: Vec<String>,
    ) -> Result<(), AdminShopifyError> {
        let variables = queries::discount_code_bulk_deactivate::Variables { ids };
        let response = self
            .execute::<DiscountCodeBulkDeactivate>(variables)
            .await?;

        if let Some(payload) = response.discount_code_bulk_deactivate
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

    /// Bulk delete code discounts.
    ///
    /// # Arguments
    ///
    /// * `ids` - List of discount node IDs to delete
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn bulk_delete_code_discounts(
        &self,
        ids: Vec<String>,
    ) -> Result<(), AdminShopifyError> {
        let variables = queries::discount_code_bulk_delete::Variables { ids };
        let response = self.execute::<DiscountCodeBulkDelete>(variables).await?;

        if let Some(payload) = response.discount_code_bulk_delete
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

    /// Get customer segments for eligibility picker.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of segments to return
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_customer_segments(
        &self,
        first: i64,
    ) -> Result<Vec<CustomerSegment>, AdminShopifyError> {
        let variables = queries::get_customer_segments::Variables { first: Some(first) };
        let response = self.execute::<GetCustomerSegments>(variables).await?;

        let segments = response
            .segments
            .edges
            .into_iter()
            .map(|e| CustomerSegment {
                id: e.node.id,
                name: e.node.name,
            })
            .collect();

        Ok(segments)
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

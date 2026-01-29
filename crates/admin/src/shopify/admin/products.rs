//! Product CRUD operations for the Admin API.

use tracing::instrument;

use super::{
    AdminClient, AdminShopifyError, GraphQLError, ProductUpdateInput,
    conversions::{convert_product, convert_product_connection},
    queries::{
        GetProduct, GetProducts, ProductCreate, ProductDelete, ProductUpdate,
        ProductVariantsBulkUpdate,
    },
};
use crate::shopify::types::{AdminProduct, AdminProductConnection, AdminProductVariant, Money};

impl AdminClient {
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
        let variables = super::queries::get_product::Variables {
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
        let variables = super::queries::get_products::Variables {
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
        use super::queries::product_create::{ProductInput, ProductStatus, Variables};

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
    /// * `input` - Fields to update
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
        use super::queries::product_update::{ProductInput, ProductStatus, Variables};

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
        use super::queries::product_delete::{ProductDeleteInput, Variables};

        let variables = Variables {
            input: ProductDeleteInput { id: id.to_string() },
        };

        let response = self.execute::<ProductDelete>(variables).await?;

        if let Some(payload) = response.product_delete {
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
        use super::queries::product_variants_bulk_update::{
            InventoryItemInput, ProductVariantsBulkInput, Variables,
        };

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

            if let Some(variant) = payload.product_variants.and_then(|v| v.into_iter().next()) {
                return Ok(AdminProductVariant {
                    id: variant.id,
                    title: variant.title,
                    sku: variant.sku,
                    barcode: variant.barcode,
                    price: Money {
                        amount: variant.price,
                        currency_code: "USD".to_string(),
                    },
                    compare_at_price: variant.compare_at_price.map(|p| Money {
                        amount: p,
                        currency_code: "USD".to_string(),
                    }),
                    inventory_quantity: variant.inventory_quantity.unwrap_or(0),
                    inventory_item_id: String::new(),
                    inventory_management: None,
                    weight: None,
                    weight_unit: None,
                    requires_shipping: true,
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
}

//! Cart recommendations management operations for the Admin API.
//!
//! Handles reading/writing the `custom.cart_recommendations` shop metafield
//! for per-product related products configuration.

use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{AdminClient, AdminShopifyError};

/// Metafield namespace for cart recommendations.
const METAFIELD_NAMESPACE: &str = "custom";
/// Metafield key for cart recommendations.
const METAFIELD_KEY: &str = "cart_recommendations";

// =============================================================================
// Cart Recommendations Types (matches schemas/cart_recommendations.json)
// =============================================================================

/// Cart recommendations configuration stored in shop metafield.
///
/// Structure matches the JSON schema in `schemas/cart_recommendations.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CartRecommendations {
    /// Mapping of products to their related products.
    #[serde(default)]
    pub product_relations: Vec<ProductRelation>,
}

impl CartRecommendations {
    /// Get related products for a given product ID.
    ///
    /// Returns an empty slice if no relations are configured for this product.
    #[must_use]
    pub fn get_related_products(&self, product_id: &str) -> &[RelatedProduct] {
        self.product_relations
            .iter()
            .find(|r| r.product_id == product_id)
            .map_or(&[], |r| r.related_products.as_slice())
    }

    /// Set related products for a given product ID.
    ///
    /// Replaces any existing relations for this product.
    pub fn set_related_products(
        &mut self,
        product_id: String,
        related_products: Vec<RelatedProduct>,
    ) {
        // Remove existing relation if present
        self.product_relations
            .retain(|r| r.product_id != product_id);

        // Add new relation if there are related products
        if !related_products.is_empty() {
            self.product_relations.push(ProductRelation {
                product_id,
                related_products,
            });
        }
    }

    /// Remove all related products for a given product ID.
    pub fn remove_related_products(&mut self, product_id: &str) {
        self.product_relations
            .retain(|r| r.product_id != product_id);
    }
}

/// A mapping from a source product to its related products.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductRelation {
    /// Shopify product GID of the source product.
    pub product_id: String,
    /// Related products to recommend when this product is in cart.
    pub related_products: Vec<RelatedProduct>,
}

/// A related product with variant ID for add-to-cart functionality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedProduct {
    /// Shopify product GID.
    pub product_id: String,
    /// Shopify variant GID for add-to-cart.
    pub variant_id: String,
}

/// Result of fetching cart recommendations, including the compareDigest for updates.
#[derive(Debug, Clone, Default)]
pub struct CartRecommendationsWithDigest {
    /// The recommendations configuration.
    pub recommendations: CartRecommendations,
    /// The compareDigest for optimistic concurrency control.
    pub compare_digest: Option<String>,
}

// =============================================================================
// Admin Client Methods
// =============================================================================

impl AdminClient {
    /// Get the cart recommendations metafield value with compareDigest.
    ///
    /// Returns default empty recommendations if the metafield doesn't exist.
    /// The `compare_digest` should be passed to `set_cart_recommendations` for updates.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails, schema validation fails, or JSON parsing fails.
    #[instrument(skip(self))]
    pub async fn get_cart_recommendations_with_digest(
        &self,
    ) -> Result<CartRecommendationsWithDigest, AdminShopifyError> {
        use super::queries::{GetShopMetafield, get_shop_metafield::Variables};
        use super::schema_validation;

        let variables = Variables {
            namespace: METAFIELD_NAMESPACE.to_string(),
            key: METAFIELD_KEY.to_string(),
        };

        let response = self.execute::<GetShopMetafield>(variables).await?;

        let Some(metafield) = response.shop.metafield else {
            return Ok(CartRecommendationsWithDigest::default());
        };

        // Validate the JSON against the schema before parsing
        if let Err(e) = schema_validation::validate_cart_recommendations_str(&metafield.value) {
            tracing::warn!(error = %e, "Cart recommendations metafield failed schema validation");
            return Err(AdminShopifyError::ParseError(format!(
                "Schema validation failed: {e}"
            )));
        }

        let recommendations: CartRecommendations =
            serde_json::from_str(&metafield.value).map_err(|e| {
                AdminShopifyError::ParseError(format!("Invalid cart recommendations JSON: {e}"))
            })?;

        Ok(CartRecommendationsWithDigest {
            recommendations,
            compare_digest: Some(metafield.compare_digest),
        })
    }

    /// Get the cart recommendations metafield value.
    ///
    /// Returns `None` if the metafield doesn't exist or is empty.
    /// Note: Use `get_cart_recommendations_with_digest` if you need to update the metafield.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or JSON parsing fails.
    #[instrument(skip(self))]
    pub async fn get_cart_recommendations(
        &self,
    ) -> Result<Option<CartRecommendations>, AdminShopifyError> {
        let result = self.get_cart_recommendations_with_digest().await?;
        if result.recommendations.product_relations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result.recommendations))
        }
    }

    /// Set the cart recommendations metafield value.
    ///
    /// Creates the metafield if it doesn't exist.
    /// Pass the `compare_digest` from `get_cart_recommendations_with_digest` for updates.
    ///
    /// # Errors
    ///
    /// Returns an error if schema validation fails, the API request fails, or returns user errors.
    #[instrument(skip(self, recommendations))]
    pub async fn set_cart_recommendations(
        &self,
        recommendations: &CartRecommendations,
        compare_digest: Option<String>,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::{
            SetShopMetafield,
            set_shop_metafield::{MetafieldsSetInput, Variables},
        };
        use super::schema_validation;

        // Get shop ID first
        let shop_id = self.get_shop_id().await?;

        let value = serde_json::to_string(recommendations)
            .map_err(|e| AdminShopifyError::ParseError(format!("Failed to serialize: {e}")))?;

        tracing::debug!(
            metafield_value = %value,
            "Writing cart_recommendations metafield to Shopify"
        );

        tracing::info!(
            product_relations_count = recommendations.product_relations.len(),
            "Setting cart_recommendations metafield"
        );

        // Validate the JSON against the schema before writing
        if let Err(e) = schema_validation::validate_cart_recommendations_str(&value) {
            tracing::error!(error = %e, "Cart recommendations failed schema validation before write");
            return Err(AdminShopifyError::ParseError(format!(
                "Schema validation failed: {e}"
            )));
        }

        let variables = Variables {
            metafields: vec![MetafieldsSetInput {
                owner_id: shop_id,
                namespace: Some(METAFIELD_NAMESPACE.to_string()),
                key: METAFIELD_KEY.to_string(),
                value,
                type_: Some("json".to_string()),
                compare_digest,
            }],
        };

        let response = self.execute::<SetShopMetafield>(variables).await?;

        if let Some(payload) = response.metafields_set
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
}

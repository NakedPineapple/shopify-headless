//! Storefront API integration for fetching product recommendations.
//!
//! Uses the Admin API to obtain a Storefront access token, then queries
//! the Storefront API's `productRecommendations` endpoint.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

use super::{AdminClient, AdminShopifyError};

/// Title for our storefront access token.
const STOREFRONT_TOKEN_TITLE: &str = "NakedPineapple Admin Recommendations";

/// Cached storefront access token.
static STOREFRONT_TOKEN_CACHE: OnceLock<RwLock<Option<String>>> = OnceLock::new();

fn get_cache() -> &'static RwLock<Option<String>> {
    STOREFRONT_TOKEN_CACHE.get_or_init(|| RwLock::new(None))
}

// =============================================================================
// Storefront API Types
// =============================================================================

/// A product recommendation from Shopify's ML algorithm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopifyRecommendation {
    /// Product GID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// Product handle.
    pub handle: String,
    /// Featured image URL.
    pub image_url: Option<String>,
    /// First variant ID (for add-to-cart).
    pub variant_id: Option<String>,
}

/// GraphQL response for product recommendations.
#[derive(Debug, Deserialize)]
struct StorefrontResponse {
    data: Option<StorefrontData>,
    errors: Option<Vec<StorefrontError>>,
}

#[derive(Debug, Deserialize)]
struct StorefrontData {
    #[serde(rename = "productRecommendations")]
    product_recommendations: Option<Vec<StorefrontProduct>>,
}

#[derive(Debug, Deserialize)]
struct StorefrontProduct {
    id: String,
    title: String,
    handle: String,
    #[serde(rename = "featuredImage")]
    featured_image: Option<StorefrontImage>,
    variants: StorefrontVariantConnection,
}

#[derive(Debug, Deserialize)]
struct StorefrontImage {
    url: String,
}

#[derive(Debug, Deserialize)]
struct StorefrontVariantConnection {
    edges: Vec<StorefrontVariantEdge>,
}

#[derive(Debug, Deserialize)]
struct StorefrontVariantEdge {
    node: StorefrontVariant,
}

#[derive(Debug, Deserialize)]
struct StorefrontVariant {
    id: String,
}

#[derive(Debug, Deserialize)]
struct StorefrontError {
    message: String,
}

// =============================================================================
// Admin Client Methods
// =============================================================================

impl AdminClient {
    /// Get or create a Storefront access token for fetching recommendations.
    ///
    /// The token is cached in memory for the lifetime of the process.
    #[instrument(skip(self))]
    async fn get_storefront_token(&self) -> Result<String, AdminShopifyError> {
        // Check cache first
        {
            let cache = get_cache().read().await;
            if let Some(token) = cache.as_ref() {
                debug!("Using cached storefront token");
                return Ok(token.clone());
            }
        }

        // Query for existing tokens
        let token = self.fetch_or_create_storefront_token().await?;

        // Cache the token
        {
            let mut cache = get_cache().write().await;
            *cache = Some(token.clone());
        }

        Ok(token)
    }

    /// Fetch existing storefront token or create a new one.
    async fn fetch_or_create_storefront_token(&self) -> Result<String, AdminShopifyError> {
        use super::queries::{
            GetStorefrontAccessTokens, get_storefront_access_tokens::Variables as GetVariables,
        };

        debug!("Fetching existing storefront access tokens");

        let response = self
            .execute::<GetStorefrontAccessTokens>(GetVariables {})
            .await?;

        // Look for existing token with our title
        for edge in response.shop.storefront_access_tokens.edges {
            if edge.node.title == STOREFRONT_TOKEN_TITLE {
                info!("Found existing storefront token");
                return Ok(edge.node.access_token);
            }
        }

        // Create a new token
        self.create_storefront_token().await
    }

    /// Create a new storefront access token.
    async fn create_storefront_token(&self) -> Result<String, AdminShopifyError> {
        use super::queries::{
            CreateStorefrontAccessToken,
            create_storefront_access_token::{StorefrontAccessTokenInput, Variables},
        };

        info!("Creating new storefront access token");

        let variables = Variables {
            input: StorefrontAccessTokenInput {
                title: STOREFRONT_TOKEN_TITLE.to_string(),
            },
        };

        let response = self
            .execute::<CreateStorefrontAccessToken>(variables)
            .await?;

        if let Some(payload) = response.storefront_access_token_create {
            // Check for user errors
            if !payload.user_errors.is_empty() {
                let errors: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| e.message.clone())
                    .collect();
                return Err(AdminShopifyError::UserError(errors.join("; ")));
            }

            if let Some(token) = payload.storefront_access_token {
                return Ok(token.access_token);
            }
        }

        Err(AdminShopifyError::ParseError(
            "Failed to create storefront access token".to_string(),
        ))
    }

    /// Get product recommendations from Shopify's ML algorithm.
    ///
    /// Returns an empty vec if no recommendations are available (common for new stores).
    ///
    /// # Errors
    ///
    /// Returns error if the Storefront API request fails or JSON parsing fails.
    #[instrument(skip(self))]
    pub async fn get_shopify_recommendations(
        &self,
        product_id: &str,
    ) -> Result<Vec<ShopifyRecommendation>, AdminShopifyError> {
        let token = self.get_storefront_token().await?;

        // Build Storefront API URL
        let store = &self.inner.store;
        let api_version = &self.inner.api_version;
        let url = format!("https://{store}/api/{api_version}/graphql.json");

        // GraphQL query for recommendations
        let query = r"
            query GetProductRecommendations($productId: ID!) {
                productRecommendations(productId: $productId, intent: COMPLEMENTARY) {
                    id
                    title
                    handle
                    featuredImage {
                        url
                    }
                    variants(first: 1) {
                        edges {
                            node {
                                id
                            }
                        }
                    }
                }
            }
        ";

        let body = serde_json::json!({
            "query": query,
            "variables": {
                "productId": product_id
            }
        });

        debug!(product_id = %product_id, "Fetching Shopify ML recommendations");

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-Shopify-Storefront-Access-Token", &token)
            .json(&body)
            .send()
            .await
            .map_err(AdminShopifyError::Http)?;

        let response_text = response.text().await.map_err(AdminShopifyError::Http)?;

        let parsed: StorefrontResponse =
            serde_json::from_str(&response_text).map_err(AdminShopifyError::Parse)?;

        // Check for errors
        if let Some(errors) = parsed.errors
            && !errors.is_empty()
        {
            let messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            warn!(errors = ?messages, "Storefront API returned errors");
            // Don't fail - just return empty recommendations
            return Ok(Vec::new());
        }

        // Extract recommendations
        let recommendations: Vec<ShopifyRecommendation> = parsed
            .data
            .and_then(|d| d.product_recommendations)
            .unwrap_or_default()
            .into_iter()
            .map(|p| ShopifyRecommendation {
                id: p.id,
                title: p.title,
                handle: p.handle,
                image_url: p.featured_image.map(|img| img.url),
                variant_id: p.variants.edges.first().map(|e| e.node.id.clone()),
            })
            .collect();

        debug!(
            product_id = %product_id,
            count = recommendations.len(),
            "Fetched Shopify ML recommendations"
        );

        Ok(recommendations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storefront_response_parsing() {
        let json = r#"{
            "data": {
                "productRecommendations": [
                    {
                        "id": "gid://shopify/Product/123",
                        "title": "Test Product",
                        "handle": "test-product",
                        "featuredImage": {
                            "url": "https://example.com/image.jpg"
                        },
                        "variants": {
                            "edges": [
                                {
                                    "node": {
                                        "id": "gid://shopify/ProductVariant/456"
                                    }
                                }
                            ]
                        }
                    }
                ]
            }
        }"#;

        let parsed: StorefrontResponse = serde_json::from_str(json).expect("JSON should parse");
        let data = parsed.data.expect("data should be present");
        let recs = data
            .product_recommendations
            .expect("recommendations should be present");

        assert_eq!(recs.len(), 1);
        let first = recs
            .first()
            .expect("should have at least one recommendation");
        assert_eq!(first.title, "Test Product");
        assert_eq!(first.handle, "test-product");
    }

    #[test]
    fn test_empty_recommendations() {
        let json = r#"{
            "data": {
                "productRecommendations": []
            }
        }"#;

        let parsed: StorefrontResponse = serde_json::from_str(json).expect("JSON should parse");
        let data = parsed.data.expect("data should be present");
        let recs = data
            .product_recommendations
            .expect("recommendations should be present");

        assert!(recs.is_empty());
    }

    #[test]
    fn test_null_recommendations() {
        let json = r#"{
            "data": {
                "productRecommendations": null
            }
        }"#;

        let parsed: StorefrontResponse = serde_json::from_str(json).expect("JSON should parse");
        let data = parsed.data.expect("data should be present");

        assert!(data.product_recommendations.is_none());
    }
}

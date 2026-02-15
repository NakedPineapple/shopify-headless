//! Google Merchant Center Products API.

use serde::Serialize;
use tracing::instrument;

use super::GoogleMerchantError;
use super::client::GoogleMerchantClient;
use super::types::{GoogleProduct, ProductsPage};

/// Query parameters for paginated product listing.
#[derive(Serialize)]
struct ListProductsQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "maxResults")]
    max_results: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "pageToken")]
    page_token: Option<String>,
}

impl GoogleMerchantClient {
    /// List products in the merchant account.
    ///
    /// Returns up to `max_results` products per page with token-based pagination.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn list_products(
        &self,
        max_results: Option<u32>,
        page_token: Option<String>,
    ) -> Result<ProductsPage, GoogleMerchantError> {
        let merchant_id = self.merchant_id();
        let path = format!("/{merchant_id}/products");
        let params = ListProductsQuery {
            max_results: Some(max_results.unwrap_or(250)),
            page_token,
        };
        self.execute_get(&path, Some(&params)).await
    }

    /// Get a specific product by ID.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(product_id = %product_id))]
    pub async fn get_product(
        &self,
        product_id: &str,
    ) -> Result<GoogleProduct, GoogleMerchantError> {
        let merchant_id = self.merchant_id();
        let path = format!("/{merchant_id}/products/{product_id}");
        self.execute_get(&path, None::<&()>).await
    }
}

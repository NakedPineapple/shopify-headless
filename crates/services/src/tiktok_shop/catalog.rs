//! TikTok Shop Catalog API.
//!
//! Product search and retrieval via the TikTok Shop Open API.

use tracing::instrument;

use super::TikTokShopError;
use super::client::TikTokShopClient;
use super::types::{ProductSearchData, TikTokProduct};

impl TikTokShopClient {
    /// Search products in the TikTok Shop catalog.
    ///
    /// Calls `POST /api/products/search` with optional keyword filter.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails or the response cannot be parsed.
    #[instrument(skip(self), fields(query = %query))]
    pub async fn search_products(
        &self,
        query: &str,
        limit: u32,
        page_token: Option<&str>,
    ) -> Result<ProductSearchData, TikTokShopError> {
        let mut params = vec![("page_size".to_string(), limit.min(100).to_string())];

        if !query.is_empty() {
            params.push(("search_keyword".to_string(), query.to_string()));
        }
        if let Some(token) = page_token {
            params.push(("page_token".to_string(), token.to_string()));
        }

        self.execute_post("/api/products/search", &params, Option::<&()>::None)
            .await
    }

    /// Get a single product by ID.
    ///
    /// Calls `GET /api/products/{product_id}`.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(product_id = %product_id))]
    pub async fn get_product(&self, product_id: &str) -> Result<TikTokProduct, TikTokShopError> {
        let path = format!("/api/products/{product_id}");
        self.execute_get(&path, &[]).await
    }
}

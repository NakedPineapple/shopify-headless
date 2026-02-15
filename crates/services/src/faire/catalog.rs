//! Faire Products API.

use serde::Serialize;
use tracing::instrument;

use super::FaireError;
use super::client::FaireClient;
use super::types::{FaireProduct, ProductsPage};

/// Query parameters for paginated product listing.
#[derive(Serialize)]
struct ListProductsQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<i32>,
}

impl FaireClient {
    /// List products in the brand catalog.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn list_products(
        &self,
        page: Option<i32>,
        limit: Option<i32>,
    ) -> Result<ProductsPage, FaireError> {
        let params = ListProductsQuery {
            page,
            limit: Some(limit.unwrap_or(50)),
        };
        self.execute_get("/products", Some(&params)).await
    }

    /// Get a specific product by token.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(product_token = %token))]
    pub async fn get_product(&self, token: &str) -> Result<FaireProduct, FaireError> {
        let path = format!("/products/{token}");
        self.execute_get(&path, None::<&()>).await
    }
}

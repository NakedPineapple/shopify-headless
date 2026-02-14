//! Meta Commerce Catalog API.

use serde::Serialize;
use tracing::instrument;

use super::MetaCommerceError;
use super::client::MetaCommerceClient;
use super::types::{FacebookProduct, ProductsPage};

/// Fields to request for catalog products.
const PRODUCT_FIELDS: &str =
    "id,name,description,price,currency,image_url,url,retailer_id,availability,brand,category";

/// Query parameters for catalog product search.
#[derive(Serialize)]
struct CatalogSearchQuery {
    fields: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<String>,
    limit: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<String>,
}

impl MetaCommerceClient {
    /// Search products in the catalog.
    ///
    /// Calls `GET /{catalog_id}/products` with an optional name filter.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails or the response cannot be parsed.
    #[instrument(skip(self), fields(query = %query))]
    pub async fn search_products(
        &self,
        query: &str,
        limit: u32,
        after: Option<String>,
    ) -> Result<ProductsPage, MetaCommerceError> {
        let catalog_id = self.catalog_id().to_string();
        let path = format!("/{catalog_id}/products");

        let filter = if query.is_empty() {
            None
        } else {
            Some(format!("{{\"name\":{{\"i_contains\":\"{query}\"}}}}"))
        };

        let params = CatalogSearchQuery {
            fields: PRODUCT_FIELDS.to_string(),
            filter,
            limit: limit.min(100),
            after,
        };

        self.execute(&path, Some(&params)).await
    }

    /// Get a single product by ID from the catalog.
    ///
    /// Calls `GET /{product_id}` with product fields.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(product_id = %product_id))]
    pub async fn get_product(
        &self,
        product_id: &str,
    ) -> Result<FacebookProduct, MetaCommerceError> {
        let path = format!("/{product_id}");

        let params = [("fields", PRODUCT_FIELDS)];

        self.execute(&path, Some(&params)).await
    }
}

//! Amazon SP-API Catalog Items API (v2022-04-01).

use tracing::instrument;

use super::AmazonSpError;
use super::client::AmazonSpClient;
use super::types::{CatalogItem, CatalogSearchQuery, CatalogSearchResponse};

impl AmazonSpClient {
    /// Search the Amazon catalog by keywords.
    ///
    /// Calls `GET /catalog/2022-04-01/items` with the given query.
    /// Rate limit: 2 requests/second.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails or the response cannot be parsed.
    #[instrument(skip(self, keywords), fields(keywords = %keywords))]
    pub async fn search_catalog_items(
        &self,
        keywords: &str,
        page_token: Option<String>,
    ) -> Result<CatalogSearchResponse, AmazonSpError> {
        let query = CatalogSearchQuery {
            keywords: keywords.to_string(),
            marketplace_ids: self.marketplace_id().to_string(),
            included_data: Some("summaries,images,identifiers,salesRanks".to_string()),
            page_size: Some(20),
            page_token,
        };

        self.execute("/catalog/2022-04-01/items", Some(&query))
            .await
    }

    /// Get a single catalog item by ASIN.
    ///
    /// Calls `GET /catalog/2022-04-01/items/{asin}`.
    /// Rate limit: 2 requests/second.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails or the response cannot be parsed.
    #[instrument(skip(self), fields(asin = %asin))]
    pub async fn get_catalog_item(&self, asin: &str) -> Result<CatalogItem, AmazonSpError> {
        let path = format!("/catalog/2022-04-01/items/{asin}");

        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Query {
            marketplace_ids: String,
            included_data: String,
        }

        let query = Query {
            marketplace_ids: self.marketplace_id().to_string(),
            included_data: "summaries,images,identifiers,salesRanks".to_string(),
        };

        self.execute(&path, Some(&query)).await
    }
}

//! Pinterest Catalogs API.

use serde::Serialize;
use tracing::instrument;

use super::PinterestError;
use super::client::PinterestClient;
use super::types::{
    CatalogItemsPage, CatalogsPage, FeedProcessingResultsPage, FeedsPage, PinterestFeed,
};

/// Query parameters for paginated catalog endpoints.
#[derive(Serialize)]
struct PaginatedQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    bookmark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_size: Option<u32>,
}

/// Query parameters for listing catalogs.
#[derive(Serialize)]
struct CatalogsQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    bookmark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_size: Option<u32>,
}

impl PinterestClient {
    /// List catalogs (filtered to RETAIL type).
    ///
    /// # Errors
    ///
    /// Returns error if the request fails or the response cannot be parsed.
    #[instrument(skip(self))]
    pub async fn list_catalogs(
        &self,
        bookmark: Option<String>,
    ) -> Result<CatalogsPage, PinterestError> {
        let params = CatalogsQuery {
            bookmark,
            page_size: Some(25),
        };
        self.execute_get("/catalogs", Some(&params)).await
    }

    /// List product feeds.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn list_feeds(&self, bookmark: Option<String>) -> Result<FeedsPage, PinterestError> {
        let params = PaginatedQuery {
            bookmark,
            page_size: Some(25),
        };
        self.execute_get("/catalogs/feeds", Some(&params)).await
    }

    /// Get details of a specific feed.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(feed_id = %feed_id))]
    pub async fn get_feed(&self, feed_id: &str) -> Result<PinterestFeed, PinterestError> {
        let path = format!("/catalogs/feeds/{feed_id}");
        self.execute_get(&path, None::<&()>).await
    }

    /// Get processing results for a feed.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(feed_id = %feed_id))]
    pub async fn get_feed_processing_results(
        &self,
        feed_id: &str,
        bookmark: Option<String>,
    ) -> Result<FeedProcessingResultsPage, PinterestError> {
        let path = format!("/catalogs/feeds/{feed_id}/processing_results");
        let params = PaginatedQuery {
            bookmark,
            page_size: Some(25),
        };
        self.execute_get(&path, Some(&params)).await
    }

    /// Trigger feed ingestion.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(feed_id = %feed_id))]
    pub async fn trigger_feed_ingestion(
        &self,
        feed_id: &str,
    ) -> Result<serde_json::Value, PinterestError> {
        let path = format!("/catalogs/feeds/{feed_id}/ingest");
        self.execute_post(&path, &serde_json::json!({})).await
    }

    /// List products in a product group.
    ///
    /// Returns up to 250 items per page with bookmark-based pagination.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(product_group_id = %product_group_id))]
    pub async fn list_product_group_items(
        &self,
        product_group_id: &str,
        bookmark: Option<String>,
    ) -> Result<CatalogItemsPage, PinterestError> {
        let path = format!("/catalogs/product_groups/{product_group_id}/products");
        let params = PaginatedQuery {
            bookmark,
            page_size: Some(250),
        };
        self.execute_get(&path, Some(&params)).await
    }
}

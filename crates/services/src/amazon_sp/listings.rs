//! Amazon SP-API Listings Items API (v2021-08-01).

use tracing::instrument;

use super::AmazonSpError;
use super::client::AmazonSpClient;
use super::types::{
    ListingItem, ListingsItemPatchRequest, ListingsItemPutRequest, ListingsItemSubmissionResponse,
};

/// Query parameters for listings endpoints.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ListingsQuery {
    marketplace_ids: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    included_data: Option<String>,
}

impl AmazonSpClient {
    /// Get a listing item by SKU.
    ///
    /// Calls `GET /listings/2021-08-01/items/{sellerId}/{sku}`.
    /// Rate limit: 5 requests/second.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails or the response cannot be parsed.
    #[instrument(skip(self), fields(sku = %sku))]
    pub async fn get_listing(&self, sku: &str) -> Result<ListingItem, AmazonSpError> {
        let seller_id = self.seller_id();
        let encoded_sku = urlencoding::encode(sku);
        let path = format!("/listings/2021-08-01/items/{seller_id}/{encoded_sku}");

        let query = ListingsQuery {
            marketplace_ids: self.marketplace_id().to_string(),
            included_data: Some("summaries,issues,offers,fulfillmentAvailability".to_string()),
        };

        self.execute(&path, Some(&query)).await
    }

    /// Create or fully replace a listing.
    ///
    /// Calls `PUT /listings/2021-08-01/items/{sellerId}/{sku}`.
    /// Rate limit: 5 requests/second.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self, body), fields(sku = %sku))]
    pub async fn put_listing(
        &self,
        sku: &str,
        body: &ListingsItemPutRequest,
    ) -> Result<ListingsItemSubmissionResponse, AmazonSpError> {
        let seller_id = self.seller_id();
        let encoded_sku = urlencoding::encode(sku);
        let path = format!("/listings/2021-08-01/items/{seller_id}/{encoded_sku}");

        let query = ListingsQuery {
            marketplace_ids: self.marketplace_id().to_string(),
            included_data: None,
        };

        self.execute_with_retry(reqwest::Method::PUT, &path, Some(&query), Some(body))
            .await
    }

    /// Partially update a listing.
    ///
    /// Calls `PATCH /listings/2021-08-01/items/{sellerId}/{sku}`.
    /// Rate limit: 5 requests/second.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self, body), fields(sku = %sku))]
    pub async fn patch_listing(
        &self,
        sku: &str,
        body: &ListingsItemPatchRequest,
    ) -> Result<ListingsItemSubmissionResponse, AmazonSpError> {
        let seller_id = self.seller_id();
        let encoded_sku = urlencoding::encode(sku);
        let path = format!("/listings/2021-08-01/items/{seller_id}/{encoded_sku}");

        let query = ListingsQuery {
            marketplace_ids: self.marketplace_id().to_string(),
            included_data: None,
        };

        self.execute_with_retry(reqwest::Method::PATCH, &path, Some(&query), Some(body))
            .await
    }

    /// Delete a listing.
    ///
    /// Calls `DELETE /listings/2021-08-01/items/{sellerId}/{sku}`.
    /// Rate limit: 5 requests/second.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(sku = %sku))]
    pub async fn delete_listing(
        &self,
        sku: &str,
    ) -> Result<ListingsItemSubmissionResponse, AmazonSpError> {
        let seller_id = self.seller_id();
        let encoded_sku = urlencoding::encode(sku);
        let path = format!("/listings/2021-08-01/items/{seller_id}/{encoded_sku}");

        let query = ListingsQuery {
            marketplace_ids: self.marketplace_id().to_string(),
            included_data: None,
        };

        self.execute_with_retry(
            reqwest::Method::DELETE,
            &path,
            Some(&query),
            Option::<&()>::None,
        )
        .await
    }
}

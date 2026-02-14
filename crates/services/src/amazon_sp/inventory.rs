//! Amazon SP-API FBA Inventory API (v1).

use tracing::instrument;

use super::AmazonSpError;
use super::client::AmazonSpClient;
use super::types::{
    GetInventorySummariesResponse, InventorySummariesQuery, InventorySummary, SpApiError,
};

impl AmazonSpClient {
    /// Get FBA inventory summaries.
    ///
    /// Calls `GET /fba/inventory/v1/summaries` with marketplace-level granularity.
    /// Rate limit: 2 requests/second.
    ///
    /// Returns all inventory summaries, following pagination automatically.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails or the response cannot be parsed.
    #[instrument(skip(self))]
    pub async fn get_inventory_summaries(&self) -> Result<Vec<InventorySummary>, AmazonSpError> {
        let mut all_summaries = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let batch = self.get_inventory_summaries_page(next_token).await?;
            all_summaries.extend(batch.summaries);

            if let Some(token) = batch.next_token {
                next_token = Some(token);
            } else {
                break;
            }
        }

        Ok(all_summaries)
    }

    /// Get a single page of FBA inventory summaries.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn get_inventory_summaries_page(
        &self,
        next_token: Option<String>,
    ) -> Result<InventorySummariesPage, AmazonSpError> {
        let marketplace_id = self.marketplace_id().to_string();

        let query = InventorySummariesQuery {
            marketplace_ids: marketplace_id.clone(),
            granularity_type: "Marketplace".to_string(),
            granularity_id: marketplace_id,
            next_token,
            seller_skus: None,
        };

        let response: GetInventorySummariesResponse = self
            .execute("/fba/inventory/v1/summaries", Some(&query))
            .await?;

        if let Some(errors) = response.errors {
            return Err(map_inventory_errors(&errors));
        }

        let summaries = response
            .payload
            .map(|p| p.inventory_summaries)
            .unwrap_or_default();

        let page_next_token = response.pagination.and_then(|p| p.next_token);

        Ok(InventorySummariesPage {
            summaries,
            next_token: page_next_token,
        })
    }
}

/// A page of inventory summaries with optional pagination token.
pub struct InventorySummariesPage {
    /// Inventory summaries in this page.
    pub summaries: Vec<InventorySummary>,
    /// Token for the next page, if any.
    pub next_token: Option<String>,
}

/// Map SP-API error array to our error type.
fn map_inventory_errors(errors: &[SpApiError]) -> AmazonSpError {
    errors.first().map_or(
        AmazonSpError::Parse("Unknown inventory API error".to_string()),
        |e| AmazonSpError::Api {
            status: 400,
            message: e.message.clone(),
        },
    )
}

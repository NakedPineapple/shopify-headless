//! TikTok Shop Finance / Settlement API.
//!
//! Settlement listing and details via the TikTok Shop Open API.

use tracing::instrument;

use super::TikTokShopError;
use super::client::TikTokShopClient;
use super::types::{SettlementListData, TikTokSettlement};

impl TikTokShopClient {
    /// Get settlements with optional pagination.
    ///
    /// Calls `GET /api/finance/settlements`.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn get_settlements(
        &self,
        page_size: u32,
        page_token: Option<&str>,
    ) -> Result<SettlementListData, TikTokShopError> {
        let mut params = vec![("page_size".to_string(), page_size.min(50).to_string())];

        if let Some(token) = page_token {
            params.push(("page_token".to_string(), token.to_string()));
        }

        self.execute_get("/api/finance/settlements", &params).await
    }

    /// Get settlement details by ID.
    ///
    /// Calls `GET /api/finance/settlements/{settlement_id}`.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(settlement_id = %settlement_id))]
    pub async fn get_settlement_details(
        &self,
        settlement_id: &str,
    ) -> Result<TikTokSettlement, TikTokShopError> {
        let path = format!("/api/finance/settlements/{settlement_id}");
        self.execute_get(&path, &[]).await
    }
}

//! TikTok Shop info and performance metrics.
//!
//! Shop information and health/performance data via the TikTok Shop Open API.

use tracing::instrument;

use super::TikTokShopError;
use super::client::TikTokShopClient;
use super::types::{ShopInfoData, ShopPerformance};

impl TikTokShopClient {
    /// Get authorized shop information.
    ///
    /// Calls `GET /api/shop/get_authorized_shop`.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn get_shop_info(&self) -> Result<ShopInfoData, TikTokShopError> {
        self.execute_get("/api/shop/get_authorized_shop", &[]).await
    }

    /// Get shop performance metrics.
    ///
    /// Calls `GET /api/shop/performance`.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn get_performance_metrics(&self) -> Result<ShopPerformance, TikTokShopError> {
        self.execute_get("/api/shop/performance", &[]).await
    }
}

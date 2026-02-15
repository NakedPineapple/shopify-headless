//! TikTok Shop Orders API.
//!
//! Order listing, details, and paginated fetch via the TikTok Shop Open API.

use tracing::instrument;

use super::TikTokShopError;
use super::client::TikTokShopClient;
use super::types::{OrderListData, TikTokOrder};

impl TikTokShopClient {
    /// Get orders with optional filters.
    ///
    /// Calls `POST /api/orders/search` with time range and status filters.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn get_orders(
        &self,
        status: Option<&str>,
        create_time_from: Option<i64>,
        create_time_to: Option<i64>,
        page_size: u32,
        page_token: Option<&str>,
    ) -> Result<OrderListData, TikTokShopError> {
        let mut params = vec![("page_size".to_string(), page_size.min(100).to_string())];

        if let Some(s) = status {
            params.push(("order_status".to_string(), s.to_string()));
        }
        if let Some(from) = create_time_from {
            params.push(("create_time_from".to_string(), from.to_string()));
        }
        if let Some(to) = create_time_to {
            params.push(("create_time_to".to_string(), to.to_string()));
        }
        if let Some(token) = page_token {
            params.push(("page_token".to_string(), token.to_string()));
        }

        self.execute_post("/api/orders/search", &params, Option::<&()>::None)
            .await
    }

    /// Get a single order by ID.
    ///
    /// Calls `GET /api/orders/{order_id}`.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn get_order_details(&self, order_id: &str) -> Result<TikTokOrder, TikTokShopError> {
        let path = format!("/api/orders/{order_id}");
        self.execute_get(&path, &[]).await
    }

    /// Get all orders since a given timestamp, following pagination.
    ///
    /// # Errors
    ///
    /// Returns error if any page request fails.
    #[instrument(skip(self))]
    pub async fn get_all_orders_since(
        &self,
        since_timestamp: i64,
    ) -> Result<Vec<TikTokOrder>, TikTokShopError> {
        let mut all_orders = Vec::new();
        let mut page_token: Option<String> = None;
        let now = chrono::Utc::now().timestamp();

        loop {
            let page = self
                .get_orders(
                    None,
                    Some(since_timestamp),
                    Some(now),
                    50,
                    page_token.as_deref(),
                )
                .await?;

            if let Some(orders) = page.orders {
                all_orders.extend(orders);
            }

            match page.next_page_token.filter(|t| !t.is_empty()) {
                Some(token) => page_token = Some(token),
                None => break,
            }
        }

        Ok(all_orders)
    }
}

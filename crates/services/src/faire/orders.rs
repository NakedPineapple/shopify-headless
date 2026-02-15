//! Faire Orders API.

use serde::Serialize;
use tracing::instrument;

use super::FaireError;
use super::client::FaireClient;
use super::types::{FaireOrder, OrdersPage};

/// Query parameters for listing orders.
#[derive(Serialize)]
struct ListOrdersQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
}

/// Query parameters for fetching orders since a timestamp.
#[derive(Serialize)]
struct OrdersSinceQuery {
    since: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<i32>,
}

impl FaireClient {
    /// List orders with optional status filter.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn list_orders(
        &self,
        page: Option<i32>,
        limit: Option<i32>,
        state: Option<String>,
    ) -> Result<OrdersPage, FaireError> {
        let params = ListOrdersQuery {
            page,
            limit: Some(limit.unwrap_or(50)),
            state,
        };
        self.execute_get("/orders", Some(&params)).await
    }

    /// Get a specific order by token.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(order_token = %token))]
    pub async fn get_order(&self, token: &str) -> Result<FaireOrder, FaireError> {
        let path = format!("/orders/{token}");
        self.execute_get(&path, None::<&()>).await
    }

    /// Fetch all orders since a timestamp, paginating through all results.
    ///
    /// # Errors
    ///
    /// Returns error if any page request fails.
    #[instrument(skip(self))]
    pub async fn get_all_orders_since(&self, since: &str) -> Result<Vec<FaireOrder>, FaireError> {
        let mut all_orders = Vec::new();
        let mut current_page = 1;

        loop {
            let params = OrdersSinceQuery {
                since: since.to_string(),
                page: Some(current_page),
                limit: Some(50),
            };

            let page: OrdersPage = self.execute_get("/orders", Some(&params)).await?;
            let orders = page.orders.unwrap_or_default();
            all_orders.extend(orders);

            if page.has_more.unwrap_or(false) {
                current_page += 1;
            } else {
                break;
            }
        }

        Ok(all_orders)
    }
}

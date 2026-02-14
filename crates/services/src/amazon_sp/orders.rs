//! Amazon SP-API Orders API (v0).

use tracing::instrument;

use super::AmazonSpError;
use super::client::AmazonSpClient;
use super::types::{
    AmazonOrder, AmazonOrderItem, GetOrderItemsResponse, GetOrderResponse, GetOrdersQuery,
    GetOrdersResponse, SpApiError,
};

impl AmazonSpClient {
    /// Get orders with filtering.
    ///
    /// Calls `GET /orders/v0/orders`. Rate limit: 1 request per minute.
    /// Returns a page of orders with an optional `next_token` for pagination.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails or the response cannot be parsed.
    #[instrument(skip(self))]
    pub async fn get_orders(
        &self,
        created_after: Option<&str>,
        statuses: Option<&str>,
        next_token: Option<String>,
    ) -> Result<OrdersPage, AmazonSpError> {
        let query = GetOrdersQuery {
            marketplace_ids: self.marketplace_id().to_string(),
            created_after: created_after.map(String::from),
            order_statuses: statuses.map(String::from),
            fulfillment_channels: None,
            next_token,
            max_results_per_page: Some(100),
        };

        let response: GetOrdersResponse = self.execute("/orders/v0/orders", Some(&query)).await?;

        if let Some(errors) = response.errors {
            return Err(map_order_errors(&errors));
        }

        let payload = response
            .payload
            .ok_or_else(|| AmazonSpError::Parse("Missing orders payload".into()))?;

        Ok(OrdersPage {
            orders: payload.orders,
            next_token: payload.next_token,
        })
    }

    /// Get all orders created after a timestamp, following pagination.
    ///
    /// # Errors
    ///
    /// Returns error if any page request fails.
    #[instrument(skip(self))]
    pub async fn get_all_orders_since(
        &self,
        created_after: &str,
    ) -> Result<Vec<AmazonOrder>, AmazonSpError> {
        let mut all_orders = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let page = self
                .get_orders(Some(created_after), None, next_token)
                .await?;
            all_orders.extend(page.orders);

            if let Some(token) = page.next_token {
                next_token = Some(token);
            } else {
                break;
            }
        }

        Ok(all_orders)
    }

    /// Get a single order by ID.
    ///
    /// Calls `GET /orders/v0/orders/{orderId}`. Rate limit: 1 request per second.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn get_order(&self, order_id: &str) -> Result<AmazonOrder, AmazonSpError> {
        let path = format!("/orders/v0/orders/{order_id}");

        let response: GetOrderResponse = self.execute(&path, None::<&()>).await?;

        if let Some(errors) = response.errors {
            return Err(map_order_errors(&errors));
        }

        response
            .payload
            .ok_or_else(|| AmazonSpError::Parse("Missing order payload".into()))
    }

    /// Get order items for an order.
    ///
    /// Calls `GET /orders/v0/orders/{orderId}/orderItems`. Rate limit: 1 request
    /// per 2 seconds. Follows pagination automatically.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn get_order_items(
        &self,
        order_id: &str,
    ) -> Result<Vec<AmazonOrderItem>, AmazonSpError> {
        let path = format!("/orders/v0/orders/{order_id}/orderItems");
        let mut all_items = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let response: GetOrderItemsResponse = if let Some(ref token) = next_token {
                let query = [("NextToken", token.as_str())];
                self.execute(&path, Some(&query)).await?
            } else {
                self.execute(&path, None::<&()>).await?
            };

            if let Some(errors) = response.errors {
                return Err(map_order_errors(&errors));
            }

            if let Some(payload) = response.payload {
                all_items.extend(payload.order_items);
                if let Some(token) = payload.next_token {
                    next_token = Some(token);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(all_items)
    }
}

/// A page of orders with optional pagination token.
pub struct OrdersPage {
    /// Orders in this page.
    pub orders: Vec<AmazonOrder>,
    /// Token for the next page, if any.
    pub next_token: Option<String>,
}

/// Map SP-API error array to our error type.
fn map_order_errors(errors: &[SpApiError]) -> AmazonSpError {
    errors.first().map_or(
        AmazonSpError::Parse("Unknown orders API error".to_string()),
        |e| AmazonSpError::Api {
            status: 400,
            message: e.message.clone(),
        },
    )
}

//! Meta Commerce Orders API.

use serde::Serialize;
use tracing::instrument;

use super::MetaCommerceError;
use super::client::MetaCommerceClient;
use super::types::{FacebookOrder, OrdersPage};

/// Fields to request for orders.
const ORDER_FIELDS: &str = "id,order_status,created,last_updated,channel,\
    selected_shipping_option,shipping_address,estimated_payment_details,\
    buyer_details,items{id,product_id,retailer_id,quantity,price_per_unit,tax_details}";

/// Query parameters for order listing.
#[derive(Serialize)]
struct OrdersQuery {
    fields: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_after: Option<String>,
    limit: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<String>,
}

impl MetaCommerceClient {
    /// Get orders from the commerce account.
    ///
    /// Calls `GET /{commerce_account_id}/orders` with optional filters.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn get_orders(
        &self,
        state: Option<&str>,
        updated_after: Option<&str>,
        limit: u32,
        after: Option<String>,
    ) -> Result<OrdersPage, MetaCommerceError> {
        let account_id = self.commerce_account_id().to_string();
        let path = format!("/{account_id}/orders");

        let params = OrdersQuery {
            fields: ORDER_FIELDS.to_string(),
            state: state.map(String::from),
            updated_after: updated_after.map(String::from),
            limit: limit.min(100),
            after,
        };

        self.execute(&path, Some(&params)).await
    }

    /// Get a single order by ID.
    ///
    /// Calls `GET /{order_id}` with order fields.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn get_order_details(
        &self,
        order_id: &str,
    ) -> Result<FacebookOrder, MetaCommerceError> {
        let path = format!("/{order_id}");

        let params = [("fields", ORDER_FIELDS)];

        self.execute(&path, Some(&params)).await
    }

    /// Get all orders since a given timestamp, following pagination.
    ///
    /// # Errors
    ///
    /// Returns error if any page request fails.
    #[instrument(skip(self))]
    pub async fn get_all_orders_since(
        &self,
        updated_after: &str,
    ) -> Result<Vec<FacebookOrder>, MetaCommerceError> {
        let mut all_orders = Vec::new();
        let mut after_cursor: Option<String> = None;

        loop {
            let page = self
                .get_orders(None, Some(updated_after), 50, after_cursor)
                .await?;

            all_orders.extend(page.data);

            match page.paging.and_then(|p| p.cursors).and_then(|c| c.after) {
                Some(cursor) => after_cursor = Some(cursor),
                None => break,
            }
        }

        Ok(all_orders)
    }
}

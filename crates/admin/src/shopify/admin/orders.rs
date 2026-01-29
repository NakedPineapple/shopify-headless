//! Order management operations for the Admin API.

use tracing::instrument;

use super::{
    AdminClient, AdminShopifyError, GraphQLError,
    conversions::{convert_order, convert_order_connection, convert_order_list_connection},
    queries::{
        GetOrder, GetOrderDetail, GetOrders, OrderCancel, OrderCapture, OrderClose,
        OrderMarkAsPaid, OrderOpen, OrderTagsAdd, OrderTagsRemove, OrderUpdate,
    },
};
use crate::shopify::types::{Order, OrderConnection, OrderListConnection, OrderSortKey};

impl AdminClient {
    /// Get an order by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID (e.g., `gid://shopify/Order/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn get_order(&self, id: &str) -> Result<Option<Order>, AdminShopifyError> {
        let variables = super::queries::get_order::Variables {
            id: id.to_string(),
            line_item_count: Some(50),
            fulfillment_count: Some(10),
        };

        let response = self.execute::<GetOrder>(variables).await?;

        Ok(response.order.map(convert_order))
    }

    /// Get a paginated list of orders.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of orders to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self))]
    pub async fn get_orders(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
    ) -> Result<OrderConnection, AdminShopifyError> {
        let variables = super::queries::get_orders::Variables {
            first: Some(first),
            after,
            query,
            sort_key: None,
            reverse: Some(false),
        };

        let response = self.execute::<GetOrders>(variables).await?;

        Ok(convert_order_connection(response.orders))
    }

    /// Get detailed order information for the order detail page.
    ///
    /// Returns extended order data including transactions, fulfillments, refunds,
    /// returns, timeline events, and customer info.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID (e.g., `gid://shopify/Order/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn get_order_detail(
        &self,
        id: &str,
    ) -> Result<Option<super::queries::get_order_detail::GetOrderDetailOrder>, AdminShopifyError>
    {
        let variables = super::queries::get_order_detail::Variables {
            id: id.to_string(),
            line_item_count: Some(100),
            fulfillment_count: Some(50),
            transaction_count: Some(50),
            event_count: Some(100),
        };

        let response = self.execute::<GetOrderDetail>(variables).await?;

        Ok(response.order)
    }

    /// Get a paginated list of orders with extended fields for data table display.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of orders to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query (Shopify query syntax)
    /// * `sort_key` - Optional sort key
    /// * `reverse` - Whether to reverse the sort order
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self))]
    pub async fn get_orders_list(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
        sort_key: Option<OrderSortKey>,
        reverse: bool,
    ) -> Result<OrderListConnection, AdminShopifyError> {
        let variables = super::queries::get_orders::Variables {
            first: Some(first),
            after,
            query,
            sort_key: sort_key.map(|k| match k {
                OrderSortKey::OrderNumber => {
                    super::queries::get_orders::OrderSortKeys::ORDER_NUMBER
                }
                OrderSortKey::TotalPrice => super::queries::get_orders::OrderSortKeys::TOTAL_PRICE,
                OrderSortKey::CreatedAt => super::queries::get_orders::OrderSortKeys::CREATED_AT,
                OrderSortKey::ProcessedAt => {
                    super::queries::get_orders::OrderSortKeys::PROCESSED_AT
                }
                OrderSortKey::UpdatedAt => super::queries::get_orders::OrderSortKeys::UPDATED_AT,
                OrderSortKey::CustomerName => {
                    super::queries::get_orders::OrderSortKeys::CUSTOMER_NAME
                }
                OrderSortKey::FinancialStatus => {
                    super::queries::get_orders::OrderSortKeys::FINANCIAL_STATUS
                }
                OrderSortKey::FulfillmentStatus => {
                    super::queries::get_orders::OrderSortKeys::FULFILLMENT_STATUS
                }
                OrderSortKey::Destination => super::queries::get_orders::OrderSortKeys::DESTINATION,
                OrderSortKey::Id => super::queries::get_orders::OrderSortKeys::ID,
            }),
            reverse: Some(reverse),
        };

        let response = self.execute::<GetOrders>(variables).await?;

        Ok(convert_order_list_connection(response.orders))
    }

    /// Update an order's note.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    /// * `note` - New note content
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn update_order_note(
        &self,
        id: &str,
        note: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::order_update::{OrderInput, Variables};

        let variables = Variables {
            input: OrderInput {
                id: id.to_string(),
                note: note.map(String::from),
                tags: None,
                custom_attributes: None,
                email: None,
                localized_fields: None,
                metafields: None,
                phone: None,
                po_number: None,
                shipping_address: None,
            },
        };

        let response = self.execute::<OrderUpdate>(variables).await?;

        if let Some(payload) = response.order_update
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Update an order's tags.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    /// * `tags` - New tags (replaces existing tags)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn update_order_tags(
        &self,
        id: &str,
        tags: Vec<String>,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::order_update::{OrderInput, Variables};

        let variables = Variables {
            input: OrderInput {
                id: id.to_string(),
                note: None,
                tags: Some(tags),
                custom_attributes: None,
                email: None,
                localized_fields: None,
                metafields: None,
                phone: None,
                po_number: None,
                shipping_address: None,
            },
        };

        let response = self.execute::<OrderUpdate>(variables).await?;

        if let Some(payload) = response.order_update
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Mark an order as paid.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn mark_order_as_paid(&self, id: &str) -> Result<(), AdminShopifyError> {
        use super::queries::order_mark_as_paid::{OrderMarkAsPaidInput, Variables};

        let variables = Variables {
            input: OrderMarkAsPaidInput { id: id.to_string() },
        };

        let response = self.execute::<OrderMarkAsPaid>(variables).await?;

        if let Some(payload) = response.order_mark_as_paid
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Cancel an order.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    /// * `reason` - Cancellation reason
    /// * `notify_customer` - Whether to notify the customer
    /// * `refund` - Whether to refund the order
    /// * `restock` - Whether to restock inventory
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn cancel_order(
        &self,
        id: &str,
        reason: Option<&str>,
        notify_customer: bool,
        refund: bool,
        restock: bool,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::order_cancel::{OrderCancelReason, Variables};

        let cancel_reason = reason.map_or(OrderCancelReason::OTHER, |r| {
            match r.to_uppercase().as_str() {
                "CUSTOMER" => OrderCancelReason::CUSTOMER,
                "FRAUD" => OrderCancelReason::FRAUD,
                "INVENTORY" => OrderCancelReason::INVENTORY,
                "DECLINED" => OrderCancelReason::DECLINED,
                _ => OrderCancelReason::OTHER,
            }
        });

        let variables = Variables {
            order_id: id.to_string(),
            reason: cancel_reason,
            notify_customer: Some(notify_customer),
            refund: Some(refund),
            restock,
            staff_note: None,
        };

        let response = self.execute::<OrderCancel>(variables).await?;

        if let Some(payload) = response.order_cancel
            && !payload.order_cancel_user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .order_cancel_user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Archive (close) an order.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn archive_order(&self, id: &str) -> Result<(), AdminShopifyError> {
        use super::queries::order_close::{OrderCloseInput, Variables};

        let variables = Variables {
            input: OrderCloseInput { id: id.to_string() },
        };

        let response = self.execute::<OrderClose>(variables).await?;

        if let Some(payload) = response.order_close
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Unarchive (reopen) an order.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn unarchive_order(&self, id: &str) -> Result<(), AdminShopifyError> {
        use super::queries::order_open::{OrderOpenInput, Variables};

        let variables = Variables {
            input: OrderOpenInput { id: id.to_string() },
        };

        let response = self.execute::<OrderOpen>(variables).await?;

        if let Some(payload) = response.order_open
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Capture payment on an order.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    /// * `parent_transaction_id` - ID of the authorized transaction to capture
    /// * `amount` - Amount to capture (required)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn capture_order_payment(
        &self,
        id: &str,
        parent_transaction_id: &str,
        amount: &str,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::order_capture::{OrderCaptureInput, Variables};

        let variables = Variables {
            input: OrderCaptureInput {
                id: id.to_string(),
                amount: amount.to_string(),
                parent_transaction_id: parent_transaction_id.to_string(),
                currency: None,
                final_capture: None,
            },
        };

        let response = self.execute::<OrderCapture>(variables).await?;

        if let Some(payload) = response.order_capture
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| {
                    let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                    format!("{}: {}", field, e.message)
                })
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Add tags to an order.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    /// * `tags` - Tags to add
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn add_tags_to_order(
        &self,
        id: &str,
        tags: &[String],
    ) -> Result<Vec<String>, AdminShopifyError> {
        use super::queries::order_tags_add::Variables;

        let variables = Variables {
            id: id.to_string(),
            tags: tags.to_vec(),
        };

        let response = self.execute::<OrderTagsAdd>(variables).await?;

        if let Some(payload) = response.tags_add {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(vec![]);
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Tags add failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Remove tags from an order.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify order ID
    /// * `tags` - Tags to remove
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %id))]
    pub async fn remove_tags_from_order(
        &self,
        id: &str,
        tags: &[String],
    ) -> Result<Vec<String>, AdminShopifyError> {
        use super::queries::order_tags_remove::Variables;

        let variables = Variables {
            id: id.to_string(),
            tags: tags.to_vec(),
        };

        let response = self.execute::<OrderTagsRemove>(variables).await?;

        if let Some(payload) = response.tags_remove {
            if !payload.user_errors.is_empty() {
                let error_messages: Vec<String> = payload
                    .user_errors
                    .iter()
                    .map(|e| {
                        let field = e.field.as_ref().map_or_else(String::new, |f| f.join("."));
                        format!("{}: {}", field, e.message)
                    })
                    .collect();
                return Err(AdminShopifyError::UserError(error_messages.join("; ")));
            }

            return Ok(vec![]);
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Tags remove failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }
}

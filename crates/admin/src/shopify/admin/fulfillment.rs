//! Fulfillment, refund, hold, and return operations for the Admin API.

use tracing::instrument;

use super::{
    AdminClient, AdminShopifyError, GraphQLError,
    conversions::convert_fulfillment_orders,
    queries::{
        FulfillmentCreate, FulfillmentOrderHold, FulfillmentOrderReleaseHold,
        FulfillmentTrackingInfoUpdate, GetFulfillmentOrders, RefundCreate, ReturnCreate,
        SuggestedRefund,
    },
};
use crate::shopify::types::{
    FulfillmentHoldInput, FulfillmentHoldReason, FulfillmentOrder, RefundCreateInput,
    RefundRestockType, ReturnCreateInput, SuggestedRefundLineItem, SuggestedRefundResult,
};

impl AdminClient {
    /// Get fulfillment orders for an order.
    ///
    /// # Arguments
    ///
    /// * `order_id` - Shopify order ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn get_fulfillment_orders(
        &self,
        order_id: &str,
    ) -> Result<Vec<FulfillmentOrder>, AdminShopifyError> {
        let variables = super::queries::get_fulfillment_orders::Variables {
            order_id: order_id.to_string(),
        };

        let response = self.execute::<GetFulfillmentOrders>(variables).await?;

        Ok(convert_fulfillment_orders(response.order))
    }

    /// Create a fulfillment.
    ///
    /// # Arguments
    ///
    /// * `fulfillment_order_id` - Fulfillment order ID to fulfill
    /// * `tracking_company` - Optional shipping carrier
    /// * `tracking_number` - Optional tracking number
    /// * `tracking_url` - Optional tracking URL
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(fulfillment_order_id = %fulfillment_order_id))]
    pub async fn create_fulfillment(
        &self,
        fulfillment_order_id: &str,
        tracking_company: Option<&str>,
        tracking_number: Option<&str>,
        tracking_url: Option<&str>,
    ) -> Result<String, AdminShopifyError> {
        use super::queries::fulfillment_create::{
            FulfillmentInput, FulfillmentOrderLineItemsInput, FulfillmentTrackingInput, Variables,
        };

        let tracking_info =
            if tracking_company.is_some() || tracking_number.is_some() || tracking_url.is_some() {
                Some(FulfillmentTrackingInput {
                    company: tracking_company.map(String::from),
                    number: tracking_number.map(String::from),
                    url: tracking_url.map(String::from),
                    numbers: None,
                    urls: None,
                })
            } else {
                None
            };

        let variables = Variables {
            fulfillment: FulfillmentInput {
                line_items_by_fulfillment_order: vec![FulfillmentOrderLineItemsInput {
                    fulfillment_order_id: fulfillment_order_id.to_string(),
                    fulfillment_order_line_items: None,
                }],
                tracking_info,
                notify_customer: Some(true),
                origin_address: None,
            },
        };

        let response = self.execute::<FulfillmentCreate>(variables).await?;

        if let Some(payload) = response.fulfillment_create {
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

            if let Some(fulfillment) = payload.fulfillment {
                return Ok(fulfillment.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No fulfillment returned from create".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update fulfillment tracking info.
    ///
    /// # Arguments
    ///
    /// * `fulfillment_id` - Fulfillment ID
    /// * `tracking_company` - Shipping carrier
    /// * `tracking_number` - Tracking number
    /// * `tracking_url` - Optional tracking URL
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(fulfillment_id = %fulfillment_id))]
    pub async fn update_fulfillment_tracking(
        &self,
        fulfillment_id: &str,
        tracking_company: Option<&str>,
        tracking_number: Option<&str>,
        tracking_url: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::fulfillment_tracking_info_update::{
            FulfillmentTrackingInput, Variables,
        };

        let variables = Variables {
            fulfillment_id: fulfillment_id.to_string(),
            tracking_info_input: FulfillmentTrackingInput {
                company: tracking_company.map(String::from),
                number: tracking_number.map(String::from),
                url: tracking_url.map(String::from),
                numbers: None,
                urls: None,
            },
        };

        let response = self
            .execute::<FulfillmentTrackingInfoUpdate>(variables)
            .await?;

        if let Some(payload) = response.fulfillment_tracking_info_update
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

    /// Create a refund for an order.
    ///
    /// # Arguments
    ///
    /// * `order_id` - Shopify order ID
    /// * `input` - Refund configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input), fields(order_id = %order_id))]
    pub async fn create_refund(
        &self,
        order_id: &str,
        input: RefundCreateInput,
    ) -> Result<String, AdminShopifyError> {
        use super::queries::refund_create::{
            RefundInput, RefundLineItemInput as GqlRefundLineItemInput, RefundLineItemRestockType,
            ShippingRefundInput, Variables,
        };

        let refund_line_items: Vec<GqlRefundLineItemInput> = input
            .line_items
            .into_iter()
            .map(|item| GqlRefundLineItemInput {
                line_item_id: item.line_item_id,
                quantity: item.quantity,
                restock_type: Some(match item.restock_type {
                    RefundRestockType::Return => RefundLineItemRestockType::RETURN,
                    RefundRestockType::Cancel => RefundLineItemRestockType::CANCEL,
                    RefundRestockType::NoRestock => RefundLineItemRestockType::NO_RESTOCK,
                }),
                location_id: item.location_id,
            })
            .collect();

        let shipping = if input.full_shipping_refund || input.shipping_amount.is_some() {
            Some(ShippingRefundInput {
                full_refund: Some(input.full_shipping_refund),
                amount: input.shipping_amount,
            })
        } else {
            None
        };

        let variables = Variables {
            input: RefundInput {
                order_id: order_id.to_string(),
                note: input.note,
                notify: Some(input.notify),
                refund_line_items: Some(refund_line_items),
                shipping,
                currency: None,
                processed_at: None,
                refund_duties: None,
                transactions: None,
                refund_methods: None,
                discrepancy_reason: None,
                allow_over_refunding: None,
            },
        };

        let response = self.execute::<RefundCreate>(variables).await?;

        if let Some(payload) = response.refund_create {
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

            if let Some(refund) = payload.refund {
                return Ok(refund.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No refund returned from create".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Get suggested refund calculation for an order.
    ///
    /// # Arguments
    ///
    /// * `order_id` - Shopify order ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or the order is not found.
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn get_suggested_refund(
        &self,
        order_id: &str,
    ) -> Result<SuggestedRefundResult, AdminShopifyError> {
        let variables = super::queries::suggested_refund::Variables {
            order_id: order_id.to_string(),
        };

        let response = self.execute::<SuggestedRefund>(variables).await?;

        let order = response.order.ok_or_else(|| {
            AdminShopifyError::GraphQL(vec![GraphQLError {
                message: "Order not found".to_string(),
                locations: vec![],
                path: vec![],
            }])
        })?;

        let suggested = order.suggested_refund.ok_or_else(|| {
            AdminShopifyError::GraphQL(vec![GraphQLError {
                message: "No suggested refund available".to_string(),
                locations: vec![],
                path: vec![],
            }])
        })?;

        let amount = suggested.amount_set.shop_money.amount.clone();
        let currency_code = format!("{:?}", suggested.amount_set.shop_money.currency_code);
        let subtotal = suggested.subtotal_set.shop_money.amount.clone();
        let total_tax = suggested.total_tax_set.shop_money.amount.clone();

        let line_items = suggested
            .refund_line_items
            .into_iter()
            .map(|item| SuggestedRefundLineItem {
                line_item_id: item.line_item.id,
                title: item.line_item.title,
                original_quantity: item.line_item.quantity,
                refund_quantity: item.quantity,
            })
            .collect();

        Ok(SuggestedRefundResult {
            amount,
            currency_code,
            subtotal,
            total_tax,
            line_items,
        })
    }

    /// Hold a fulfillment order.
    ///
    /// Prevents the fulfillment order from being fulfilled until the hold is released.
    ///
    /// # Arguments
    ///
    /// * `fulfillment_order_id` - Fulfillment order ID to hold
    /// * `input` - Hold configuration (reason, notes, notify)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input), fields(fulfillment_order_id = %fulfillment_order_id))]
    pub async fn hold_fulfillment_order(
        &self,
        fulfillment_order_id: &str,
        input: FulfillmentHoldInput,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::fulfillment_order_hold::{
            FulfillmentHoldReason as GqlReason, FulfillmentOrderHoldInput, Variables,
        };

        let reason = match input.reason {
            FulfillmentHoldReason::AwaitingPayment => GqlReason::AWAITING_PAYMENT,
            FulfillmentHoldReason::HighRiskOfFraud => GqlReason::HIGH_RISK_OF_FRAUD,
            FulfillmentHoldReason::IncorrectAddress => GqlReason::INCORRECT_ADDRESS,
            FulfillmentHoldReason::InventoryOutOfStock => GqlReason::INVENTORY_OUT_OF_STOCK,
            FulfillmentHoldReason::UnknownDeliveryDate => GqlReason::UNKNOWN_DELIVERY_DATE,
            FulfillmentHoldReason::AwaitingReturnItems => GqlReason::AWAITING_RETURN_ITEMS,
            FulfillmentHoldReason::Other => GqlReason::OTHER,
        };

        let variables = Variables {
            id: fulfillment_order_id.to_string(),
            fulfillment_hold: FulfillmentOrderHoldInput {
                reason,
                reason_notes: input.reason_notes,
                notify_merchant: Some(input.notify_merchant),
                external_id: None,
                handle: None,
                fulfillment_order_line_items: None,
            },
        };

        let response = self.execute::<FulfillmentOrderHold>(variables).await?;

        if let Some(payload) = response.fulfillment_order_hold
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

    /// Release a hold on a fulfillment order.
    ///
    /// # Arguments
    ///
    /// * `fulfillment_order_id` - Fulfillment order ID to release
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(fulfillment_order_id = %fulfillment_order_id))]
    pub async fn release_fulfillment_order_hold(
        &self,
        fulfillment_order_id: &str,
    ) -> Result<(), AdminShopifyError> {
        let variables = super::queries::fulfillment_order_release_hold::Variables {
            id: fulfillment_order_id.to_string(),
        };

        let response = self
            .execute::<FulfillmentOrderReleaseHold>(variables)
            .await?;

        if let Some(payload) = response.fulfillment_order_release_hold
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

    /// Create a return for an order.
    ///
    /// # Arguments
    ///
    /// * `order_id` - Shopify order ID
    /// * `input` - Return configuration (line items to return)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input), fields(order_id = %order_id))]
    pub async fn create_return(
        &self,
        order_id: &str,
        input: ReturnCreateInput,
    ) -> Result<String, AdminShopifyError> {
        use super::queries::return_create::{ReturnInput, ReturnLineItemInput, Variables};

        let return_line_items: Vec<ReturnLineItemInput> = input
            .line_items
            .into_iter()
            .map(|item| ReturnLineItemInput {
                fulfillment_line_item_id: item.fulfillment_line_item_id,
                quantity: item.quantity,
                return_reason_note: item.return_reason_note,
                return_reason_definition_id: None,
                restocking_fee: None,
            })
            .collect();

        let variables = Variables {
            return_input: ReturnInput {
                order_id: order_id.to_string(),
                return_line_items,
                requested_at: input.requested_at,
                exchange_line_items: None,
                return_shipping_fee: None,
            },
        };

        let response = self.execute::<ReturnCreate>(variables).await?;

        if let Some(payload) = response.return_create {
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

            if let Some(ret) = payload.return_ {
                return Ok(ret.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No return created".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }
}

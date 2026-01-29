//! Order editing operations for the Admin API.

use tracing::instrument;

use super::{
    AdminClient, AdminShopifyError, GraphQLError,
    conversions::convert_calculated_order,
    queries::{
        OrderEditAddCustomItem, OrderEditAddLineItemDiscount, OrderEditAddShippingLine,
        OrderEditAddVariant, OrderEditBegin, OrderEditCommit, OrderEditRemoveDiscount,
        OrderEditRemoveShippingLine, OrderEditSetQuantity, OrderEditUpdateDiscount,
        OrderEditUpdateShippingLine,
    },
};
use crate::shopify::types::{
    CalculatedOrder, Money, OrderEditAddShippingLineInput, OrderEditAppliedDiscountInput,
    OrderEditUpdateShippingLineInput,
};

impl AdminClient {
    /// Begin an order edit session.
    ///
    /// This starts a new order edit session and returns a `CalculatedOrder`
    /// which tracks the proposed changes until they are committed.
    ///
    /// # Arguments
    ///
    /// * `order_id` - Shopify order ID (e.g., `gid://shopify/Order/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn order_edit_begin(
        &self,
        order_id: &str,
    ) -> Result<CalculatedOrder, AdminShopifyError> {
        let variables = super::queries::order_edit_begin::Variables {
            id: order_id.to_string(),
        };

        let response = self.execute::<OrderEditBegin>(variables).await?;

        if let Some(payload) = response.order_edit_begin {
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

            if let Some(calc_order) = payload.calculated_order {
                return Ok(convert_calculated_order(calc_order));
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Order edit begin failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Add a product variant to an order edit.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `variant_id` - Shopify product variant ID
    /// * `quantity` - Quantity to add
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(calc_order_id = %calculated_order_id, variant_id = %variant_id))]
    pub async fn order_edit_add_variant(
        &self,
        calculated_order_id: &str,
        variant_id: &str,
        quantity: i64,
    ) -> Result<(), AdminShopifyError> {
        let variables = super::queries::order_edit_add_variant::Variables {
            id: calculated_order_id.to_string(),
            variant_id: variant_id.to_string(),
            quantity,
            location_id: None,
            allow_duplicates: Some(false),
        };

        let response = self.execute::<OrderEditAddVariant>(variables).await?;

        if let Some(payload) = response.order_edit_add_variant {
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
            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Order edit add variant failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Add a custom line item to an order edit.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `title` - Title for the custom item
    /// * `quantity` - Quantity to add
    /// * `price` - Unit price for the item
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(calc_order_id = %calculated_order_id, title = %title))]
    pub async fn order_edit_add_custom_item(
        &self,
        calculated_order_id: &str,
        title: &str,
        quantity: i64,
        price: &Money,
        taxable: bool,
        requires_shipping: bool,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::order_edit_add_custom_item::MoneyInput;

        let variables = super::queries::order_edit_add_custom_item::Variables {
            id: calculated_order_id.to_string(),
            title: title.to_string(),
            quantity,
            price: MoneyInput {
                amount: price.amount.clone(),
                currency_code: super::queries::order_edit_add_custom_item::CurrencyCode::USD,
            },
            location_id: None,
            taxable: Some(taxable),
            requires_shipping: Some(requires_shipping),
        };

        let response = self.execute::<OrderEditAddCustomItem>(variables).await?;

        if let Some(payload) = response.order_edit_add_custom_item {
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
            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Order edit add custom item failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Set the quantity of a line item in an order edit.
    ///
    /// Set quantity to 0 to remove the item.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `line_item_id` - The calculated line item ID
    /// * `quantity` - New quantity (0 to remove)
    /// * `restock` - Whether to restock items when reducing quantity
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(calc_order_id = %calculated_order_id, line_item_id = %line_item_id))]
    pub async fn order_edit_set_quantity(
        &self,
        calculated_order_id: &str,
        line_item_id: &str,
        quantity: i64,
        restock: bool,
    ) -> Result<(), AdminShopifyError> {
        let variables = super::queries::order_edit_set_quantity::Variables {
            id: calculated_order_id.to_string(),
            line_item_id: line_item_id.to_string(),
            quantity,
            restock: Some(restock),
        };

        let response = self.execute::<OrderEditSetQuantity>(variables).await?;

        if let Some(payload) = response.order_edit_set_quantity {
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
            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Order edit set quantity failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Add a discount to a line item in an order edit.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `line_item_id` - The calculated line item ID
    /// * `discount` - The discount to apply
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, discount), fields(calc_order_id = %calculated_order_id, line_item_id = %line_item_id))]
    pub async fn order_edit_add_line_item_discount(
        &self,
        calculated_order_id: &str,
        line_item_id: &str,
        discount: &OrderEditAppliedDiscountInput,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::order_edit_add_line_item_discount::{
            MoneyInput, OrderEditAppliedDiscountInput as GqlDiscount,
        };

        let gql_discount = GqlDiscount {
            description: discount.description.clone(),
            fixed_value: discount.fixed_value.as_ref().map(|m| MoneyInput {
                amount: m.amount.clone(),
                currency_code: super::queries::order_edit_add_line_item_discount::CurrencyCode::USD,
            }),
            percent_value: discount.percent_value,
        };

        let variables = super::queries::order_edit_add_line_item_discount::Variables {
            id: calculated_order_id.to_string(),
            line_item_id: line_item_id.to_string(),
            discount: gql_discount,
        };

        let response = self
            .execute::<OrderEditAddLineItemDiscount>(variables)
            .await?;

        if let Some(payload) = response.order_edit_add_line_item_discount {
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
            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Order edit add line item discount failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update an existing discount in an order edit.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `discount_application_id` - The discount application ID to update
    /// * `discount` - The new discount values
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, discount), fields(calc_order_id = %calculated_order_id, discount_id = %discount_application_id))]
    pub async fn order_edit_update_discount(
        &self,
        calculated_order_id: &str,
        discount_application_id: &str,
        discount: &OrderEditAppliedDiscountInput,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::order_edit_update_discount::{
            MoneyInput, OrderEditAppliedDiscountInput as GqlDiscount,
        };

        let gql_discount = GqlDiscount {
            description: discount.description.clone(),
            fixed_value: discount.fixed_value.as_ref().map(|m| MoneyInput {
                amount: m.amount.clone(),
                currency_code: super::queries::order_edit_update_discount::CurrencyCode::USD,
            }),
            percent_value: discount.percent_value,
        };

        let variables = super::queries::order_edit_update_discount::Variables {
            id: calculated_order_id.to_string(),
            discount_application_id: discount_application_id.to_string(),
            discount: gql_discount,
        };

        let response = self.execute::<OrderEditUpdateDiscount>(variables).await?;

        if let Some(payload) = response.order_edit_update_discount {
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
            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Order edit update discount failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Remove a discount from an order edit.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `discount_application_id` - The discount application ID to remove
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(calc_order_id = %calculated_order_id, discount_id = %discount_application_id))]
    pub async fn order_edit_remove_discount(
        &self,
        calculated_order_id: &str,
        discount_application_id: &str,
    ) -> Result<(), AdminShopifyError> {
        let variables = super::queries::order_edit_remove_discount::Variables {
            id: calculated_order_id.to_string(),
            discount_application_id: discount_application_id.to_string(),
        };

        let response = self.execute::<OrderEditRemoveDiscount>(variables).await?;

        if let Some(payload) = response.order_edit_remove_discount {
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
            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Order edit remove discount failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Add a shipping line to an order edit.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `input` - Shipping line details (title and price)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input), fields(calc_order_id = %calculated_order_id))]
    pub async fn order_edit_add_shipping_line(
        &self,
        calculated_order_id: &str,
        input: &OrderEditAddShippingLineInput,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::order_edit_add_shipping_line::{
            MoneyInput, OrderEditAddShippingLineInput as GqlInput,
        };

        let gql_input = GqlInput {
            title: input.title.clone(),
            price: MoneyInput {
                amount: input.price.amount.clone(),
                currency_code: super::queries::order_edit_add_shipping_line::CurrencyCode::USD,
            },
        };

        let variables = super::queries::order_edit_add_shipping_line::Variables {
            id: calculated_order_id.to_string(),
            shipping_line: gql_input,
        };

        let response = self.execute::<OrderEditAddShippingLine>(variables).await?;

        if let Some(payload) = response.order_edit_add_shipping_line {
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
            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Order edit add shipping line failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Update a shipping line in an order edit.
    ///
    /// Only staged (newly added) shipping lines can be updated.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `shipping_line_id` - The shipping line ID to update
    /// * `input` - New shipping line details
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input), fields(calc_order_id = %calculated_order_id, shipping_line_id = %shipping_line_id))]
    pub async fn order_edit_update_shipping_line(
        &self,
        calculated_order_id: &str,
        shipping_line_id: &str,
        input: &OrderEditUpdateShippingLineInput,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::order_edit_update_shipping_line::{
            MoneyInput, OrderEditUpdateShippingLineInput as GqlInput,
        };

        let gql_input = GqlInput {
            title: input.title.clone(),
            price: input.price.as_ref().map(|p| MoneyInput {
                amount: p.amount.clone(),
                currency_code: super::queries::order_edit_update_shipping_line::CurrencyCode::USD,
            }),
        };

        let variables = super::queries::order_edit_update_shipping_line::Variables {
            id: calculated_order_id.to_string(),
            shipping_line_id: shipping_line_id.to_string(),
            shipping_line: gql_input,
        };

        let response = self
            .execute::<OrderEditUpdateShippingLine>(variables)
            .await?;

        if let Some(payload) = response.order_edit_update_shipping_line {
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
            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Order edit update shipping line failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Remove a shipping line from an order edit.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `shipping_line_id` - The shipping line ID to remove
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(calc_order_id = %calculated_order_id, shipping_line_id = %shipping_line_id))]
    pub async fn order_edit_remove_shipping_line(
        &self,
        calculated_order_id: &str,
        shipping_line_id: &str,
    ) -> Result<(), AdminShopifyError> {
        let variables = super::queries::order_edit_remove_shipping_line::Variables {
            id: calculated_order_id.to_string(),
            shipping_line_id: shipping_line_id.to_string(),
        };

        let response = self
            .execute::<OrderEditRemoveShippingLine>(variables)
            .await?;

        if let Some(payload) = response.order_edit_remove_shipping_line {
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
            return Ok(());
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Order edit remove shipping line failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Commit an order edit, finalizing all changes.
    ///
    /// This applies all staged changes to the order. If the edit changes
    /// the total, the customer may need to pay a balance or receive a refund.
    ///
    /// # Arguments
    ///
    /// * `calculated_order_id` - The ID from `order_edit_begin`
    /// * `notify_customer` - Whether to notify the customer about the changes
    /// * `staff_note` - Optional internal note about the edit
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(calc_order_id = %calculated_order_id))]
    pub async fn order_edit_commit(
        &self,
        calculated_order_id: &str,
        notify_customer: bool,
        staff_note: Option<&str>,
    ) -> Result<String, AdminShopifyError> {
        let variables = super::queries::order_edit_commit::Variables {
            id: calculated_order_id.to_string(),
            notify_customer: Some(notify_customer),
            staff_note: staff_note.map(String::from),
        };

        let response = self.execute::<OrderEditCommit>(variables).await?;

        if let Some(payload) = response.order_edit_commit {
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

            if let Some(order) = payload.order {
                return Ok(order.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "Order edit commit failed".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }
}

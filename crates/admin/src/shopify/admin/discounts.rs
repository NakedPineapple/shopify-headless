//! Discount management operations for the Admin API.

use tracing::instrument;

use super::{
    AdminClient, AdminShopifyError, DiscountCreateInput, DiscountUpdateInput, GraphQLError,
    queries::{
        DiscountAutomaticActivate, DiscountAutomaticDeactivate, DiscountAutomaticDelete,
        DiscountCodeActivate, DiscountCodeBasicCreate, DiscountCodeBasicUpdate,
        DiscountCodeBulkActivate, DiscountCodeBulkDeactivate, DiscountCodeBulkDelete,
        DiscountCodeDeactivate, DiscountCodeDelete, GetCustomerSegments, GetDiscountCode,
        GetDiscountCodes, GetDiscountNodes,
    },
};
use crate::shopify::types::{
    CustomerSegment, DiscountCode, DiscountCodeConnection, DiscountCombinesWith,
    DiscountListConnection, DiscountListItem, DiscountMethod, DiscountMinimumRequirement,
    DiscountSortKey, DiscountStatus, DiscountType, DiscountValue, PageInfo,
};

/// Convert GraphQL discount status to domain type.
const fn convert_discount_status(
    status: &super::queries::get_discount_codes::DiscountStatus,
) -> DiscountStatus {
    match status {
        super::queries::get_discount_codes::DiscountStatus::ACTIVE
        | super::queries::get_discount_codes::DiscountStatus::Other(_) => DiscountStatus::Active,
        super::queries::get_discount_codes::DiscountStatus::EXPIRED => DiscountStatus::Expired,
        super::queries::get_discount_codes::DiscountStatus::SCHEDULED => DiscountStatus::Scheduled,
    }
}

/// Convert GraphQL discount status to domain type (for single discount query).
const fn convert_discount_status_single(
    status: &super::queries::get_discount_code::DiscountStatus,
) -> DiscountStatus {
    match status {
        super::queries::get_discount_code::DiscountStatus::ACTIVE
        | super::queries::get_discount_code::DiscountStatus::Other(_) => DiscountStatus::Active,
        super::queries::get_discount_code::DiscountStatus::EXPIRED => DiscountStatus::Expired,
        super::queries::get_discount_code::DiscountStatus::SCHEDULED => DiscountStatus::Scheduled,
    }
}

impl AdminClient {
    /// Get a paginated list of discount codes.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of discounts to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    #[allow(deprecated)]
    pub async fn get_discounts(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
    ) -> Result<DiscountCodeConnection, AdminShopifyError> {
        let variables = super::queries::get_discount_codes::Variables {
            first: Some(first),
            after,
            query,
        };

        let response = self.execute::<GetDiscountCodes>(variables).await?;

        let discount_codes: Vec<DiscountCode> = response
            .discount_nodes
            .edges
            .into_iter()
            .filter_map(|e| {
                let node = e.node;
                let cd = node.discount;

                match cd {
                    super::queries::get_discount_codes::GetDiscountCodesDiscountNodesEdgesNodeDiscount::DiscountCodeBasic(basic) => {
                        use super::queries::get_discount_codes::GetDiscountCodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBasicCustomerGetsValue as BasicValue;
                        let code = basic.codes.edges.first().map(|e2| e2.node.code.clone()).unwrap_or_default();
                        let value = match basic.customer_gets.value {
                            BasicValue::DiscountPercentage(p) => {
                                Some(DiscountValue::Percentage { percentage: p.percentage })
                            }
                            BasicValue::DiscountAmount(a) => {
                                Some(DiscountValue::FixedAmount {
                                    amount: a.amount.amount,
                                    currency: format!("{:?}", a.amount.currency_code),
                                })
                            }
                            BasicValue::DiscountOnQuantity => None,
                        };

                        Some(DiscountCode {
                            id: node.id,
                            title: basic.title,
                            code,
                            status: convert_discount_status(&basic.status),
                            starts_at: Some(basic.starts_at),
                            ends_at: basic.ends_at,
                            usage_limit: basic.usage_limit,
                            usage_count: basic.async_usage_count,
                            value,
                        })
                    }
                    super::queries::get_discount_codes::GetDiscountCodesDiscountNodesEdgesNodeDiscount::DiscountCodeBxgy(bxgy) => {
                        let code = bxgy.codes.edges.first().map(|e2| e2.node.code.clone()).unwrap_or_default();
                        Some(DiscountCode {
                            id: node.id,
                            title: bxgy.title,
                            code,
                            status: convert_discount_status(&bxgy.status),
                            starts_at: Some(bxgy.starts_at),
                            ends_at: bxgy.ends_at,
                            usage_limit: bxgy.usage_limit,
                            usage_count: bxgy.async_usage_count,
                            value: None,
                        })
                    }
                    super::queries::get_discount_codes::GetDiscountCodesDiscountNodesEdgesNodeDiscount::DiscountCodeFreeShipping(fs) => {
                        let code = fs.codes.edges.first().map(|e2| e2.node.code.clone()).unwrap_or_default();
                        Some(DiscountCode {
                            id: node.id,
                            title: fs.title,
                            code,
                            status: convert_discount_status(&fs.status),
                            starts_at: Some(fs.starts_at),
                            ends_at: fs.ends_at,
                            usage_limit: fs.usage_limit,
                            usage_count: fs.async_usage_count,
                            value: None,
                        })
                    }
                    _ => None,
                }
            })
            .collect();

        Ok(DiscountCodeConnection {
            discount_codes,
            page_info: PageInfo {
                has_next_page: response.discount_nodes.page_info.has_next_page,
                has_previous_page: false,
                start_cursor: None,
                end_cursor: response.discount_nodes.page_info.end_cursor,
            },
        })
    }

    /// Create a basic discount code (percentage or fixed amount).
    ///
    /// # Arguments
    ///
    /// * `input` - Discount creation parameters
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input))]
    pub async fn create_discount(
        &self,
        input: DiscountCreateInput<'_>,
    ) -> Result<String, AdminShopifyError> {
        use super::queries::discount_code_basic_create::{
            DiscountCodeBasicInput, DiscountCustomerGetsInput, DiscountCustomerGetsValueInput,
            DiscountItemsInput, Variables,
        };

        let value = if let Some(pct) = input.percentage {
            DiscountCustomerGetsValueInput {
                percentage: Some(pct),
                discount_amount: None,
                discount_on_quantity: None,
            }
        } else if let Some((amt, _currency)) = input.amount {
            use super::queries::discount_code_basic_create::DiscountAmountInput;
            DiscountCustomerGetsValueInput {
                percentage: None,
                discount_amount: Some(DiscountAmountInput {
                    amount: Some(amt.to_string()),
                    applies_on_each_item: Some(false),
                }),
                discount_on_quantity: None,
            }
        } else {
            return Err(AdminShopifyError::UserError(
                "Must specify either percentage or amount".to_string(),
            ));
        };

        let variables = Variables {
            basic_code_discount: DiscountCodeBasicInput {
                title: Some(input.title.to_string()),
                code: Some(input.code.to_string()),
                starts_at: Some(input.starts_at.to_string()),
                ends_at: input.ends_at.map(String::from),
                usage_limit: input.usage_limit,
                customer_gets: Some(DiscountCustomerGetsInput {
                    value: Some(value),
                    items: Some(DiscountItemsInput {
                        all: Some(true),
                        collections: None,
                        products: None,
                    }),
                    applies_on_one_time_purchase: None,
                    applies_on_subscription: None,
                }),
                applies_once_per_customer: Some(false),
                combines_with: None,
                minimum_requirement: None,
                recurring_cycle_limit: None,
                context: None,
            },
        };

        let response = self.execute::<DiscountCodeBasicCreate>(variables).await?;

        if let Some(payload) = response.discount_code_basic_create {
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

            if let Some(node) = payload.code_discount_node {
                return Ok(node.id);
            }
        }

        Err(AdminShopifyError::GraphQL(vec![GraphQLError {
            message: "No discount returned from create".to_string(),
            locations: vec![],
            path: vec![],
        }]))
    }

    /// Deactivate a discount code.
    ///
    /// # Arguments
    ///
    /// * `id` - Discount node ID to deactivate
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn deactivate_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        let variables = super::queries::discount_code_deactivate::Variables { id: id.to_string() };

        let response = self.execute::<DiscountCodeDeactivate>(variables).await?;

        if let Some(payload) = response.discount_code_deactivate
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

    /// Get a single discount code by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Discount node ID (e.g., `gid://shopify/DiscountCodeNode/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or the discount is not found.
    #[instrument(skip(self), fields(discount_id = %id))]
    pub async fn get_discount(&self, id: &str) -> Result<DiscountCode, AdminShopifyError> {
        let variables = super::queries::get_discount_code::Variables { id: id.to_string() };

        let response = self.execute::<GetDiscountCode>(variables).await?;

        let Some(node) = response.discount_node else {
            return Err(AdminShopifyError::NotFound(format!(
                "Discount {id} not found"
            )));
        };

        use super::queries::get_discount_code::GetDiscountCodeDiscountNodeDiscount as Discount;
        match node.discount {
            Discount::DiscountCodeBasic(basic) => {
                use super::queries::get_discount_code::GetDiscountCodeDiscountNodeDiscountOnDiscountCodeBasicCustomerGetsValue as BasicValue;
                let code = basic
                    .codes
                    .edges
                    .first()
                    .map(|e| e.node.code.clone())
                    .unwrap_or_default();
                let value = match basic.customer_gets.value {
                    BasicValue::DiscountPercentage(p) => Some(DiscountValue::Percentage {
                        percentage: p.percentage,
                    }),
                    BasicValue::DiscountAmount(a) => Some(DiscountValue::FixedAmount {
                        amount: a.amount.amount,
                        currency: format!("{:?}", a.amount.currency_code),
                    }),
                    BasicValue::DiscountOnQuantity => None,
                };

                Ok(DiscountCode {
                    id: node.id,
                    title: basic.title,
                    code,
                    status: convert_discount_status_single(&basic.status),
                    starts_at: Some(basic.starts_at),
                    ends_at: basic.ends_at,
                    usage_limit: basic.usage_limit,
                    usage_count: basic.async_usage_count,
                    value,
                })
            }
            _ => Err(AdminShopifyError::NotFound(format!(
                "Discount {id} is not a basic discount code (BXGY, Free Shipping, and automatic discounts cannot be edited here)"
            ))),
        }
    }

    /// Update a basic discount code.
    ///
    /// # Arguments
    ///
    /// * `id` - Discount node ID
    /// * `input` - Update parameters
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input), fields(discount_id = %id))]
    pub async fn update_discount(
        &self,
        id: &str,
        input: DiscountUpdateInput<'_>,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::discount_code_basic_update::{DiscountCodeBasicInput, Variables};

        let variables = Variables {
            id: id.to_string(),
            basic_code_discount: DiscountCodeBasicInput {
                title: input.title.map(String::from),
                code: None,
                starts_at: input.starts_at.map(String::from),
                ends_at: input.ends_at.map(String::from),
                usage_limit: None,
                customer_gets: None,
                applies_once_per_customer: None,
                combines_with: None,
                minimum_requirement: None,
                recurring_cycle_limit: None,
                context: None,
            },
        };

        let response = self.execute::<DiscountCodeBasicUpdate>(variables).await?;

        if let Some(payload) = response.discount_code_basic_update
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

    /// Get a paginated list of discounts with sorting/filtering (all types).
    ///
    /// # Arguments
    ///
    /// * `first` - Number of discounts to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query
    /// * `sort_key` - Sort key
    /// * `reverse` - Reverse sort order
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_discounts_for_list(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
        sort_key: Option<DiscountSortKey>,
        reverse: bool,
    ) -> Result<DiscountListConnection, AdminShopifyError> {
        use super::queries::get_discount_nodes::{DiscountSortKeys, Variables};

        let gql_sort_key = sort_key.map(|sk| match sk {
            DiscountSortKey::Title => DiscountSortKeys::TITLE,
            DiscountSortKey::CreatedAt => DiscountSortKeys::CREATED_AT,
            DiscountSortKey::UpdatedAt => DiscountSortKeys::UPDATED_AT,
            DiscountSortKey::StartsAt => DiscountSortKeys::STARTS_AT,
            DiscountSortKey::EndsAt => DiscountSortKeys::ENDS_AT,
            DiscountSortKey::Id => DiscountSortKeys::ID,
        });

        let variables = Variables {
            first: Some(first),
            after,
            query,
            sort_key: gql_sort_key,
            reverse: Some(reverse),
        };

        let response = self.execute::<GetDiscountNodes>(variables).await?;

        let discounts = Self::convert_discount_nodes_to_list(response.discount_nodes);

        Ok(discounts)
    }

    /// Convert GraphQL discount nodes to list items.
    fn convert_discount_nodes_to_list(
        nodes: super::queries::get_discount_nodes::GetDiscountNodesDiscountNodes,
    ) -> DiscountListConnection {
        use super::queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscount as Discount;

        let discounts: Vec<DiscountListItem> = nodes
            .edges
            .into_iter()
            .filter_map(|edge| {
                let id = edge.node.id;
                Self::convert_single_discount_node(id, edge.node.discount)
            })
            .collect();

        DiscountListConnection {
            discounts,
            page_info: PageInfo {
                has_next_page: nodes.page_info.has_next_page,
                has_previous_page: nodes.page_info.has_previous_page,
                start_cursor: nodes.page_info.start_cursor,
                end_cursor: nodes.page_info.end_cursor,
            },
        }
    }

    /// Convert a single discount node to a list item.
    fn convert_single_discount_node(
        id: String,
        discount: super::queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscount,
    ) -> Option<DiscountListItem> {
        use super::queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscount as Discount;

        match discount {
            Discount::DiscountCodeBasic(basic) => Some(Self::convert_code_basic_node(id, basic)),
            Discount::DiscountCodeBxgy(bxgy) => Some(Self::convert_code_bxgy_node(id, bxgy)),
            Discount::DiscountCodeFreeShipping(fs) => {
                Some(Self::convert_code_freeshipping_node(id, fs))
            }
            Discount::DiscountAutomaticBasic(auto) => Some(Self::convert_auto_basic_node(id, auto)),
            Discount::DiscountAutomaticBxgy(auto) => Some(Self::convert_auto_bxgy_node(id, auto)),
            Discount::DiscountAutomaticFreeShipping(auto) => {
                Some(Self::convert_auto_freeshipping_node(id, auto))
            }
            _ => None,
        }
    }

    /// Convert code basic discount to list item.
    fn convert_code_basic_node(
        id: String,
        basic: super::queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBasic,
    ) -> DiscountListItem {
        let code = basic.codes.edges.first().map(|e| e.node.code.clone());
        let value = Self::convert_discount_value_nodes(&basic.customer_gets.value);
        let minimum = Self::convert_minimum_requirement_nodes(basic.minimum_requirement.as_ref());
        let combines_with = Self::convert_combines_with_nodes(&basic.combines_with);
        let code_count = i64::try_from(basic.codes.edges.len()).unwrap_or(0);

        DiscountListItem {
            id,
            title: basic.title,
            code,
            code_count,
            method: DiscountMethod::Code,
            discount_type: DiscountType::Basic,
            status: Self::convert_status_nodes(&basic.status),
            value,
            starts_at: Some(basic.starts_at),
            ends_at: basic.ends_at,
            usage_limit: basic.usage_limit,
            usage_count: basic.async_usage_count,
            once_per_customer: basic.applies_once_per_customer,
            combines_with,
            minimum_requirement: minimum,
        }
    }

    /// Convert code BXGY discount to list item.
    fn convert_code_bxgy_node(
        id: String,
        bxgy: super::queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBxgy,
    ) -> DiscountListItem {
        let code = bxgy.codes.edges.first().map(|e| e.node.code.clone());
        let combines_with = Self::convert_combines_with_nodes(&bxgy.combines_with);
        let code_count = i64::try_from(bxgy.codes.edges.len()).unwrap_or(0);

        DiscountListItem {
            id,
            title: bxgy.title,
            code,
            code_count,
            method: DiscountMethod::Code,
            discount_type: DiscountType::BuyXGetY,
            status: Self::convert_status_nodes(&bxgy.status),
            value: None,
            starts_at: Some(bxgy.starts_at),
            ends_at: bxgy.ends_at,
            usage_limit: bxgy.usage_limit,
            usage_count: bxgy.async_usage_count,
            once_per_customer: false,
            combines_with,
            minimum_requirement: DiscountMinimumRequirement::None,
        }
    }

    /// Convert code free shipping discount to list item.
    fn convert_code_freeshipping_node(
        id: String,
        fs: super::queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeFreeShipping,
    ) -> DiscountListItem {
        let code = fs.codes.edges.first().map(|e| e.node.code.clone());
        let minimum = Self::convert_minimum_requirement_nodes(fs.minimum_requirement.as_ref());
        let combines_with = Self::convert_combines_with_nodes(&fs.combines_with);
        let code_count = i64::try_from(fs.codes.edges.len()).unwrap_or(0);

        DiscountListItem {
            id,
            title: fs.title,
            code,
            code_count,
            method: DiscountMethod::Code,
            discount_type: DiscountType::FreeShipping,
            status: Self::convert_status_nodes(&fs.status),
            value: None,
            starts_at: Some(fs.starts_at),
            ends_at: fs.ends_at,
            usage_limit: fs.usage_limit,
            usage_count: fs.async_usage_count,
            once_per_customer: fs.applies_once_per_customer,
            combines_with,
            minimum_requirement: minimum,
        }
    }

    /// Convert automatic basic discount to list item.
    fn convert_auto_basic_node(
        id: String,
        auto: super::queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBasic,
    ) -> DiscountListItem {
        let value = Self::convert_discount_value_nodes(&auto.customer_gets.value);
        let minimum = Self::convert_minimum_requirement_nodes(auto.minimum_requirement.as_ref());
        let combines_with = Self::convert_combines_with_nodes(&auto.combines_with);

        DiscountListItem {
            id,
            title: auto.title,
            code: None,
            code_count: 0,
            method: DiscountMethod::Automatic,
            discount_type: DiscountType::Basic,
            status: Self::convert_status_nodes(&auto.status),
            value,
            starts_at: Some(auto.starts_at),
            ends_at: auto.ends_at,
            usage_limit: None,
            usage_count: auto.async_usage_count,
            once_per_customer: false,
            combines_with,
            minimum_requirement: minimum,
        }
    }

    /// Convert automatic BXGY discount to list item.
    fn convert_auto_bxgy_node(
        id: String,
        auto: super::queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountAutomaticBxgy,
    ) -> DiscountListItem {
        let combines_with = Self::convert_combines_with_nodes(&auto.combines_with);

        DiscountListItem {
            id,
            title: auto.title,
            code: None,
            code_count: 0,
            method: DiscountMethod::Automatic,
            discount_type: DiscountType::BuyXGetY,
            status: Self::convert_status_nodes(&auto.status),
            value: None,
            starts_at: Some(auto.starts_at),
            ends_at: auto.ends_at,
            usage_limit: None,
            usage_count: auto.async_usage_count,
            once_per_customer: false,
            combines_with,
            minimum_requirement: DiscountMinimumRequirement::None,
        }
    }

    /// Convert automatic free shipping discount to list item.
    fn convert_auto_freeshipping_node(
        id: String,
        auto: super::queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountAutomaticFreeShipping,
    ) -> DiscountListItem {
        let minimum = Self::convert_minimum_requirement_nodes(auto.minimum_requirement.as_ref());
        let combines_with = Self::convert_combines_with_nodes(&auto.combines_with);

        DiscountListItem {
            id,
            title: auto.title,
            code: None,
            code_count: 0,
            method: DiscountMethod::Automatic,
            discount_type: DiscountType::FreeShipping,
            status: Self::convert_status_nodes(&auto.status),
            value: None,
            starts_at: Some(auto.starts_at),
            ends_at: auto.ends_at,
            usage_limit: None,
            usage_count: auto.async_usage_count,
            once_per_customer: false,
            combines_with,
            minimum_requirement: minimum,
        }
    }

    /// Convert discount status from GraphQL nodes query.
    const fn convert_status_nodes(
        status: &super::queries::get_discount_nodes::DiscountStatus,
    ) -> DiscountStatus {
        match status {
            super::queries::get_discount_nodes::DiscountStatus::ACTIVE
            | super::queries::get_discount_nodes::DiscountStatus::Other(_) => {
                DiscountStatus::Active
            }
            super::queries::get_discount_nodes::DiscountStatus::EXPIRED => DiscountStatus::Expired,
            super::queries::get_discount_nodes::DiscountStatus::SCHEDULED => {
                DiscountStatus::Scheduled
            }
        }
    }

    /// Convert discount value from GraphQL nodes query.
    fn convert_discount_value_nodes(
        value: &super::queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBasicCustomerGetsValue,
    ) -> Option<DiscountValue> {
        use super::queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBasicCustomerGetsValue as Value;
        match value {
            Value::DiscountPercentage(p) => Some(DiscountValue::Percentage {
                percentage: p.percentage,
            }),
            Value::DiscountAmount(a) => Some(DiscountValue::FixedAmount {
                amount: a.amount.amount.clone(),
                currency: format!("{:?}", a.amount.currency_code),
            }),
            Value::DiscountOnQuantity(_) => None,
        }
    }

    /// Convert minimum requirement from GraphQL nodes query.
    fn convert_minimum_requirement_nodes(
        req: Option<&super::queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBasicMinimumRequirement>,
    ) -> DiscountMinimumRequirement {
        use super::queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBasicMinimumRequirement as Req;
        match req {
            Some(Req::DiscountMinimumQuantity(q)) => DiscountMinimumRequirement::Quantity {
                quantity: q.greater_than_or_equal_to_quantity.clone(),
            },
            Some(Req::DiscountMinimumSubtotal(s)) => DiscountMinimumRequirement::Subtotal {
                amount: s.greater_than_or_equal_to_subtotal.amount.clone(),
                currency: format!("{:?}", s.greater_than_or_equal_to_subtotal.currency_code),
            },
            None => DiscountMinimumRequirement::None,
        }
    }

    /// Convert `combines_with` from GraphQL nodes query.
    const fn convert_combines_with_nodes(
        cw: &super::queries::get_discount_nodes::GetDiscountNodesDiscountNodesEdgesNodeDiscountOnDiscountCodeBasicCombinesWith,
    ) -> DiscountCombinesWith {
        DiscountCombinesWith {
            order_discounts: cw.order_discounts,
            product_discounts: cw.product_discounts,
            shipping_discounts: cw.shipping_discounts,
        }
    }

    /// Activate a discount (code or automatic).
    ///
    /// Detects the discount type and calls the appropriate mutation.
    ///
    /// # Arguments
    ///
    /// * `id` - Discount node ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(discount_id = %id))]
    pub async fn activate_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        let code_result = self.activate_code_discount(id).await;
        if code_result.is_ok() {
            return Ok(());
        }

        self.activate_automatic_discount(id).await
    }

    /// Activate a code discount.
    async fn activate_code_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        let variables = super::queries::discount_code_activate::Variables { id: id.to_string() };
        let response = self.execute::<DiscountCodeActivate>(variables).await?;

        if let Some(payload) = response.discount_code_activate
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

    /// Activate an automatic discount.
    async fn activate_automatic_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        let variables =
            super::queries::discount_automatic_activate::Variables { id: id.to_string() };
        let response = self.execute::<DiscountAutomaticActivate>(variables).await?;

        if let Some(payload) = response.discount_automatic_activate
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

    /// Deactivate an automatic discount.
    ///
    /// # Arguments
    ///
    /// * `id` - Automatic discount node ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(discount_id = %id))]
    pub async fn deactivate_automatic_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        let variables =
            super::queries::discount_automatic_deactivate::Variables { id: id.to_string() };
        let response = self
            .execute::<DiscountAutomaticDeactivate>(variables)
            .await?;

        if let Some(payload) = response.discount_automatic_deactivate
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

    /// Delete a discount (code or automatic).
    ///
    /// Detects the discount type and calls the appropriate mutation.
    ///
    /// # Arguments
    ///
    /// * `id` - Discount node ID
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(discount_id = %id))]
    pub async fn delete_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        let code_result = self.delete_code_discount(id).await;
        if code_result.is_ok() {
            return Ok(());
        }

        self.delete_automatic_discount(id).await
    }

    /// Delete a code discount.
    async fn delete_code_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        let variables = super::queries::discount_code_delete::Variables { id: id.to_string() };
        let response = self.execute::<DiscountCodeDelete>(variables).await?;

        if let Some(payload) = response.discount_code_delete
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

    /// Delete an automatic discount.
    async fn delete_automatic_discount(&self, id: &str) -> Result<(), AdminShopifyError> {
        let variables = super::queries::discount_automatic_delete::Variables { id: id.to_string() };
        let response = self.execute::<DiscountAutomaticDelete>(variables).await?;

        if let Some(payload) = response.discount_automatic_delete
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

    /// Bulk activate code discounts.
    ///
    /// # Arguments
    ///
    /// * `ids` - List of discount node IDs to activate
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn bulk_activate_code_discounts(
        &self,
        ids: Vec<String>,
    ) -> Result<(), AdminShopifyError> {
        let variables = super::queries::discount_code_bulk_activate::Variables { ids };
        let response = self.execute::<DiscountCodeBulkActivate>(variables).await?;

        if let Some(payload) = response.discount_code_bulk_activate
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

    /// Bulk deactivate code discounts.
    ///
    /// # Arguments
    ///
    /// * `ids` - List of discount node IDs to deactivate
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn bulk_deactivate_code_discounts(
        &self,
        ids: Vec<String>,
    ) -> Result<(), AdminShopifyError> {
        let variables = super::queries::discount_code_bulk_deactivate::Variables { ids };
        let response = self
            .execute::<DiscountCodeBulkDeactivate>(variables)
            .await?;

        if let Some(payload) = response.discount_code_bulk_deactivate
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

    /// Bulk delete code discounts.
    ///
    /// # Arguments
    ///
    /// * `ids` - List of discount node IDs to delete
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn bulk_delete_code_discounts(
        &self,
        ids: Vec<String>,
    ) -> Result<(), AdminShopifyError> {
        let variables = super::queries::discount_code_bulk_delete::Variables { ids };
        let response = self.execute::<DiscountCodeBulkDelete>(variables).await?;

        if let Some(payload) = response.discount_code_bulk_delete
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

    /// Get customer segments for eligibility picker.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of segments to return
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_customer_segments(
        &self,
        first: i64,
    ) -> Result<Vec<CustomerSegment>, AdminShopifyError> {
        let variables = super::queries::get_customer_segments::Variables { first: Some(first) };
        let response = self.execute::<GetCustomerSegments>(variables).await?;

        let segments = response
            .segments
            .edges
            .into_iter()
            .map(|e| CustomerSegment {
                id: e.node.id,
                name: e.node.name,
            })
            .collect();

        Ok(segments)
    }
}

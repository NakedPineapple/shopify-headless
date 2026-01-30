//! Executor implementations for high-level analytics tools.
//!
//! These implementations aggregate data from multiple Shopify API calls
//! to provide summarized responses for common business questions.

use std::collections::HashMap;

use serde_json::json;

use crate::claude::error::ClaudeError;
use crate::shopify::types::{CustomerSortKey, DisputeStatus};

use super::ToolExecutor;

/// Helper to safely convert i64 to usize, clamping to valid range.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn i64_to_usize(val: i64) -> usize {
    usize::try_from(val.max(0)).unwrap_or(usize::MAX)
}

impl ToolExecutor<'_> {
    // =========================================================================
    // Sales & Revenue Tools
    // =========================================================================

    pub(super) async fn get_sales_summary(
        &self,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        let start_date = input["start_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: start_date".to_string())
        })?;
        let end_date = input["end_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: end_date".to_string())
        })?;

        // Build query for date range
        let query = format!("created_at:>={start_date} created_at:<={end_date}");

        // Fetch orders with pagination to get all in range
        let mut total_sales = 0.0;
        let mut total_tax = 0.0;
        let mut total_shipping = 0.0;
        let mut total_discounts = 0.0;
        let mut order_count = 0u64;
        let mut currency = String::from("USD");
        let mut cursor: Option<String> = None;

        loop {
            let result = self
                .shopify
                .get_orders_list(50, cursor.clone(), Some(query.clone()), None, false)
                .await
                .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get orders: {e}")))?;

            for order in &result.orders {
                order_count += 1;
                total_sales += order.total_price.amount.parse::<f64>().unwrap_or(0.0);
                total_tax += order.total_tax.amount.parse::<f64>().unwrap_or(0.0);
                total_shipping += order
                    .total_shipping_price
                    .amount
                    .parse::<f64>()
                    .unwrap_or(0.0);
                total_discounts += order.total_discounts.amount.parse::<f64>().unwrap_or(0.0);
                currency.clone_from(&order.currency_code);
            }

            if !result.page_info.has_next_page {
                break;
            }
            cursor = result.page_info.end_cursor;
        }

        let gross_sales = total_sales + total_discounts;
        let net_sales = total_sales - total_tax - total_shipping;
        #[allow(clippy::cast_precision_loss)]
        let aov = if order_count > 0 {
            total_sales / order_count as f64
        } else {
            0.0
        };

        Ok(json!({
            "period": {
                "start_date": start_date,
                "end_date": end_date
            },
            "summary": {
                "total_sales": format!("{total_sales:.2} {currency}"),
                "gross_sales": format!("{gross_sales:.2} {currency}"),
                "net_sales": format!("{net_sales:.2} {currency}"),
                "total_tax": format!("{total_tax:.2} {currency}"),
                "total_shipping": format!("{total_shipping:.2} {currency}"),
                "total_discounts": format!("{total_discounts:.2} {currency}"),
                "order_count": order_count,
                "average_order_value": format!("{aov:.2} {currency}")
            }
        })
        .to_string())
    }

    pub(super) async fn get_sales_by_channel(
        &self,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        let start_date = input["start_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: start_date".to_string())
        })?;
        let end_date = input["end_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: end_date".to_string())
        })?;

        let query = format!("created_at:>={start_date} created_at:<={end_date}");

        let mut channels: HashMap<String, (i64, f64)> = HashMap::new();
        let mut currency = String::from("USD");
        let mut cursor: Option<String> = None;

        loop {
            let result = self
                .shopify
                .get_orders_list(50, cursor.clone(), Some(query.clone()), None, false)
                .await
                .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get orders: {e}")))?;

            for order in &result.orders {
                let channel = order
                    .channel_info
                    .as_ref()
                    .and_then(|ci| ci.channel_name.clone())
                    .unwrap_or_else(|| "Unknown".to_string());

                let amount = order.total_price.amount.parse::<f64>().unwrap_or(0.0);
                let entry = channels.entry(channel).or_insert((0, 0.0));
                entry.0 += 1;
                entry.1 += amount;
                currency.clone_from(&order.currency_code);
            }

            if !result.page_info.has_next_page {
                break;
            }
            cursor = result.page_info.end_cursor;
        }

        let mut channel_list: Vec<serde_json::Value> = channels
            .into_iter()
            .map(|(name, (count, revenue))| {
                json!({
                    "channel": name,
                    "order_count": count,
                    "revenue": format!("{revenue:.2} {currency}")
                })
            })
            .collect();

        // Sort by revenue descending
        channel_list.sort_by(|a, b| {
            let a_rev: f64 = a
                .get("revenue")
                .and_then(|v| v.as_str())
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let b_rev: f64 = b
                .get("revenue")
                .and_then(|v| v.as_str())
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            b_rev
                .partial_cmp(&a_rev)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(json!({
            "period": {
                "start_date": start_date,
                "end_date": end_date
            },
            "channels": channel_list
        })
        .to_string())
    }

    pub(super) async fn get_sales_by_product(
        &self,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        let start_date = input["start_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: start_date".to_string())
        })?;
        let end_date = input["end_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: end_date".to_string())
        })?;
        let limit = i64_to_usize(input["limit"].as_i64().unwrap_or(10));
        let sort_by = input["sort_by"].as_str().unwrap_or("revenue");

        let query = format!("created_at:>={start_date} created_at:<={end_date}");

        // product_title -> (units_sold, revenue)
        let mut products: HashMap<String, (i64, f64)> = HashMap::new();
        let mut currency = String::from("USD");
        let mut cursor: Option<String> = None;

        loop {
            let result = self
                .shopify
                .get_orders_list(50, cursor.clone(), Some(query.clone()), None, false)
                .await
                .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get orders: {e}")))?;

            for order in &result.orders {
                currency.clone_from(&order.currency_code);
                for line_item in &order.line_items {
                    let title = line_item.title.clone();
                    let quantity = line_item.quantity;
                    // Calculate line total: quantity * discounted_unit_price
                    let unit_price = line_item
                        .discounted_unit_price
                        .amount
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    #[allow(clippy::cast_precision_loss)]
                    let line_total = unit_price * quantity as f64;

                    let entry = products.entry(title).or_insert((0, 0.0));
                    entry.0 += quantity;
                    entry.1 += line_total;
                }
            }

            if !result.page_info.has_next_page {
                break;
            }
            cursor = result.page_info.end_cursor;
        }

        let mut product_list: Vec<serde_json::Value> = products
            .into_iter()
            .map(|(title, (units, revenue))| {
                json!({
                    "product": title,
                    "units_sold": units,
                    "revenue": format!("{revenue:.2} {currency}")
                })
            })
            .collect();

        // Sort by specified field
        product_list.sort_by(|a, b| {
            if sort_by == "units_sold" {
                let a_units = a
                    .get("units_sold")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0);
                let b_units = b
                    .get("units_sold")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0);
                b_units.cmp(&a_units)
            } else {
                let a_rev: f64 = a
                    .get("revenue")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.split_whitespace().next())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                let b_rev: f64 = b
                    .get("revenue")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.split_whitespace().next())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                b_rev
                    .partial_cmp(&a_rev)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        product_list.truncate(limit);

        Ok(json!({
            "period": {
                "start_date": start_date,
                "end_date": end_date
            },
            "sort_by": sort_by,
            "products": product_list
        })
        .to_string())
    }

    pub(super) async fn get_sales_by_location(
        &self,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        let start_date = input["start_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: start_date".to_string())
        })?;
        let end_date = input["end_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: end_date".to_string())
        })?;
        let group_by = input["group_by"].as_str().unwrap_or("country");
        let limit = i64_to_usize(input["limit"].as_i64().unwrap_or(10));

        let query = format!("created_at:>={start_date} created_at:<={end_date}");

        // location -> (order_count, revenue)
        let mut locations: HashMap<String, (i64, f64)> = HashMap::new();
        let mut currency = String::from("USD");
        let mut cursor: Option<String> = None;

        loop {
            let result = self
                .shopify
                .get_orders_list(50, cursor.clone(), Some(query.clone()), None, false)
                .await
                .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get orders: {e}")))?;

            for order in &result.orders {
                let location = order.billing_address.as_ref().map_or_else(
                    || "Unknown".to_string(),
                    |addr| {
                        if group_by == "state" {
                            format!(
                                "{}, {}",
                                addr.province_code.as_deref().unwrap_or("Unknown"),
                                addr.country_code.as_deref().unwrap_or("Unknown")
                            )
                        } else {
                            addr.country_code
                                .clone()
                                .unwrap_or_else(|| "Unknown".to_string())
                        }
                    },
                );

                let amount = order.total_price.amount.parse::<f64>().unwrap_or(0.0);
                let entry = locations.entry(location).or_insert((0, 0.0));
                entry.0 += 1;
                entry.1 += amount;
                currency.clone_from(&order.currency_code);
            }

            if !result.page_info.has_next_page {
                break;
            }
            cursor = result.page_info.end_cursor;
        }

        let mut location_list: Vec<serde_json::Value> = locations
            .into_iter()
            .map(|(name, (count, revenue))| {
                json!({
                    "location": name,
                    "order_count": count,
                    "revenue": format!("{revenue:.2} {currency}")
                })
            })
            .collect();

        // Sort by revenue descending
        location_list.sort_by(|a, b| {
            let a_rev: f64 = a
                .get("revenue")
                .and_then(|v| v.as_str())
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let b_rev: f64 = b
                .get("revenue")
                .and_then(|v| v.as_str())
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            b_rev
                .partial_cmp(&a_rev)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        location_list.truncate(limit);

        Ok(json!({
            "period": {
                "start_date": start_date,
                "end_date": end_date
            },
            "group_by": group_by,
            "locations": location_list
        })
        .to_string())
    }

    pub(super) async fn get_sales_by_discount(
        &self,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        let start_date = input["start_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: start_date".to_string())
        })?;
        let end_date = input["end_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: end_date".to_string())
        })?;
        let filter_code = input["code"].as_str();

        let query = format!("created_at:>={start_date} created_at:<={end_date}");

        // discount_code -> (usage_count, discount_amount, order_revenue)
        let mut discounts: HashMap<String, (i64, f64, f64)> = HashMap::new();
        let mut orders_with_discount = 0i64;
        let mut total_discount_amount = 0.0;
        let mut total_discounted_revenue = 0.0;
        let mut currency = String::from("USD");
        let mut cursor: Option<String> = None;

        loop {
            let result = self
                .shopify
                .get_orders_list(50, cursor.clone(), Some(query.clone()), None, false)
                .await
                .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get orders: {e}")))?;

            for order in &result.orders {
                currency.clone_from(&order.currency_code);
                let order_discount = order.total_discounts.amount.parse::<f64>().unwrap_or(0.0);
                let order_total = order.total_price.amount.parse::<f64>().unwrap_or(0.0);

                if order_discount > 0.0 {
                    orders_with_discount += 1;
                    total_discount_amount += order_discount;
                    total_discounted_revenue += order_total;

                    // Extract discount codes from the order
                    for code in &order.discount_codes {
                        if filter_code.is_some_and(|f| code.to_lowercase() != f.to_lowercase()) {
                            continue;
                        }
                        let entry = discounts.entry(code.clone()).or_insert((0, 0.0, 0.0));
                        entry.0 += 1;
                        entry.1 += order_discount;
                        entry.2 += order_total;
                    }
                }
            }

            if !result.page_info.has_next_page {
                break;
            }
            cursor = result.page_info.end_cursor;
        }

        let mut discount_list: Vec<serde_json::Value> = discounts
            .into_iter()
            .map(|(code, (count, discount, revenue))| {
                json!({
                    "code": code,
                    "usage_count": count,
                    "discount_amount": format!("{discount:.2} {currency}"),
                    "order_revenue": format!("{revenue:.2} {currency}")
                })
            })
            .collect();

        // Sort by usage count descending
        discount_list.sort_by(|a, b| {
            let a_count = a
                .get("usage_count")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            let b_count = b
                .get("usage_count")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            b_count.cmp(&a_count)
        });

        Ok(json!({
            "period": {
                "start_date": start_date,
                "end_date": end_date
            },
            "summary": {
                "orders_with_discount": orders_with_discount,
                "total_discount_amount": format!("{total_discount_amount:.2} {currency}"),
                "total_discounted_revenue": format!("{total_discounted_revenue:.2} {currency}")
            },
            "top_discount_codes": discount_list
        })
        .to_string())
    }

    // =========================================================================
    // Order Tools
    // =========================================================================

    pub(super) async fn get_order_summary(
        &self,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        let start_date = input["start_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: start_date".to_string())
        })?;
        let end_date = input["end_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: end_date".to_string())
        })?;

        let query = format!("created_at:>={start_date} created_at:<={end_date}");

        let mut total_orders = 0u64;
        let mut fulfillment_status: HashMap<String, i64> = HashMap::new();
        let mut financial_status: HashMap<String, i64> = HashMap::new();
        let mut cancelled_count = 0u64;
        let mut cursor: Option<String> = None;

        loop {
            let result = self
                .shopify
                .get_orders_list(50, cursor.clone(), Some(query.clone()), None, false)
                .await
                .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get orders: {e}")))?;

            for order in &result.orders {
                total_orders += 1;

                let fs = order
                    .fulfillment_status
                    .map_or_else(|| "UNFULFILLED".to_string(), |s| format!("{s:?}"));
                *fulfillment_status.entry(fs).or_insert(0) += 1;

                let fin_s = order
                    .financial_status
                    .map_or_else(|| "UNKNOWN".to_string(), |s| format!("{s:?}"));
                *financial_status.entry(fin_s).or_insert(0) += 1;

                if order.cancelled_at.is_some() {
                    cancelled_count += 1;
                }
            }

            if !result.page_info.has_next_page {
                break;
            }
            cursor = result.page_info.end_cursor;
        }

        #[allow(clippy::cast_precision_loss)]
        let cancellation_rate = if total_orders > 0 {
            (cancelled_count as f64 / total_orders as f64) * 100.0
        } else {
            0.0
        };

        Ok(json!({
            "period": {
                "start_date": start_date,
                "end_date": end_date
            },
            "summary": {
                "total_orders": total_orders,
                "cancelled_orders": cancelled_count,
                "cancellation_rate": format!("{cancellation_rate:.1}%")
            },
            "by_fulfillment_status": fulfillment_status,
            "by_financial_status": financial_status
        })
        .to_string())
    }

    // =========================================================================
    // Customer Tools
    // =========================================================================

    pub(super) async fn get_customer_summary(
        &self,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        let start_date = input["start_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: start_date".to_string())
        })?;
        let end_date = input["end_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: end_date".to_string())
        })?;

        let query = format!("created_at:>={start_date} created_at:<={end_date}");

        // Track unique customers and count new vs returning
        let mut customer_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut new_customers = 0u64;
        let mut cursor: Option<String> = None;

        loop {
            let result = self
                .shopify
                .get_orders_list(50, cursor.clone(), Some(query.clone()), None, false)
                .await
                .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get orders: {e}")))?;

            for order in &result.orders {
                if let Some(customer_id) = &order.customer_id {
                    // Track unique customers who ordered in this period
                    customer_ids.insert(customer_id.clone());
                }
            }

            if !result.page_info.has_next_page {
                break;
            }
            cursor = result.page_info.end_cursor;
        }

        let total_unique_customers = customer_ids.len();

        // Get new customers created in period (count by paginating)
        let new_customer_query = format!("created_at:>={start_date} created_at:<={end_date}");
        let mut new_cursor: Option<String> = None;
        loop {
            let result = self
                .shopify
                .get_customers(crate::shopify::types::CustomerListParams {
                    first: Some(250),
                    after: new_cursor.clone(),
                    query: Some(new_customer_query.clone()),
                    ..Default::default()
                })
                .await;
            match result {
                Ok(r) => {
                    new_customers += r.customers.len() as u64;
                    if !r.page_info.has_next_page {
                        break;
                    }
                    new_cursor = r.page_info.end_cursor;
                }
                Err(_) => break,
            }
        }

        // Get marketing subscriber count (count by paginating)
        let mut subscriber_count = 0u64;
        let mut sub_cursor: Option<String> = None;
        loop {
            let result = self
                .shopify
                .get_customers(crate::shopify::types::CustomerListParams {
                    first: Some(250),
                    after: sub_cursor.clone(),
                    query: Some("email_marketing_state:subscribed".to_string()),
                    ..Default::default()
                })
                .await;
            match result {
                Ok(r) => {
                    subscriber_count += r.customers.len() as u64;
                    if !r.page_info.has_next_page {
                        break;
                    }
                    sub_cursor = r.page_info.end_cursor;
                }
                Err(_) => break,
            }
        }

        Ok(json!({
            "period": {
                "start_date": start_date,
                "end_date": end_date
            },
            "summary": {
                "unique_customers_ordering": total_unique_customers,
                "new_customers_created": new_customers,
                "email_subscribers": subscriber_count
            }
        })
        .to_string())
    }

    pub(super) async fn get_top_customers(
        &self,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        let limit = input["limit"].as_i64().unwrap_or(10);
        let sort_by = input["sort_by"].as_str().unwrap_or("total_spent");

        let sort_key = if sort_by == "order_count" {
            CustomerSortKey::OrdersCount
        } else {
            CustomerSortKey::AmountSpent
        };

        let result = self
            .shopify
            .get_customers(crate::shopify::types::CustomerListParams {
                first: Some(limit),
                sort_key: Some(sort_key),
                reverse: true,
                ..Default::default()
            })
            .await
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get customers: {e}")))?;

        let customers: Vec<serde_json::Value> = result
            .customers
            .iter()
            .map(|c| {
                // Mask email for privacy
                let masked_email = c.email.as_ref().map(|e| {
                    e.find('@').map_or_else(
                        || "***".to_string(),
                        |at_pos| {
                            let (local, domain) = e.split_at(at_pos);
                            if local.len() > 2 {
                                format!("{}...{domain}", &local[..2])
                            } else {
                                format!("{local}...{domain}")
                            }
                        },
                    )
                });

                // Get last order date from recent_orders
                let last_order_date = c.recent_orders.first().map(|o| o.created_at.clone());

                json!({
                    "name": c.display_name,
                    "email": masked_email,
                    "total_spent": format!("{} {}", c.total_spent.amount, c.total_spent.currency_code),
                    "order_count": c.orders_count,
                    "last_order": last_order_date
                })
            })
            .collect();

        Ok(json!({
            "sort_by": sort_by,
            "customers": customers
        })
        .to_string())
    }

    pub(super) async fn get_customers_by_location(
        &self,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        let group_by = input["group_by"].as_str().unwrap_or("country");
        let limit = i64_to_usize(input["limit"].as_i64().unwrap_or(10));

        // Fetch customers and aggregate by location
        let mut locations: HashMap<String, i64> = HashMap::new();
        let mut cursor: Option<String> = None;
        let mut total_fetched = 0;
        let max_fetch = 500; // Limit total customers to fetch for performance

        loop {
            let result = self
                .shopify
                .get_customers(crate::shopify::types::CustomerListParams {
                    first: Some(50),
                    after: cursor.clone(),
                    ..Default::default()
                })
                .await
                .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get customers: {e}")))?;

            for customer in &result.customers {
                if let Some(addr) = &customer.default_address {
                    let location = if group_by == "state" {
                        format!(
                            "{}, {}",
                            addr.province_code.as_deref().unwrap_or("Unknown"),
                            addr.country_code.as_deref().unwrap_or("Unknown")
                        )
                    } else {
                        addr.country_code
                            .clone()
                            .unwrap_or_else(|| "Unknown".to_string())
                    };
                    *locations.entry(location).or_insert(0) += 1;
                }
            }

            total_fetched += result.customers.len();
            if !result.page_info.has_next_page || total_fetched >= max_fetch {
                break;
            }
            cursor = result.page_info.end_cursor;
        }

        let mut location_list: Vec<serde_json::Value> = locations
            .into_iter()
            .map(|(name, count)| {
                json!({
                    "location": name,
                    "customer_count": count
                })
            })
            .collect();

        // Sort by count descending
        location_list.sort_by(|a, b| {
            let a_count = a
                .get("customer_count")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            let b_count = b
                .get("customer_count")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            b_count.cmp(&a_count)
        });

        location_list.truncate(limit);

        Ok(json!({
            "group_by": group_by,
            "locations": location_list
        })
        .to_string())
    }

    // =========================================================================
    // Product & Inventory Tools
    // =========================================================================

    pub(super) async fn get_product_catalog(
        &self,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        let limit = input["limit"].as_i64().unwrap_or(20);
        let status = input["status"].as_str();
        let search_query = input["query"].as_str();

        // Build query
        let mut query_parts = Vec::new();
        if let Some(s) = status {
            query_parts.push(format!("status:{s}"));
        }
        if let Some(q) = search_query {
            query_parts.push(q.to_string());
        }
        let query = if query_parts.is_empty() {
            None
        } else {
            Some(query_parts.join(" "))
        };

        let result = self
            .shopify
            .get_products(limit, None, query)
            .await
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get products: {e}")))?;

        let products: Vec<serde_json::Value> = result
            .products
            .iter()
            .map(|p| {
                // Compute price range from variants
                let prices: Vec<f64> = p
                    .variants
                    .iter()
                    .filter_map(|v| v.price.amount.parse::<f64>().ok())
                    .collect();
                let currency = p
                    .variants
                    .first()
                    .map_or("USD", |v| v.price.currency_code.as_str());
                let price_range = if prices.is_empty() {
                    "N/A".to_string()
                } else {
                    let min = prices.iter().copied().fold(f64::INFINITY, f64::min);
                    let max = prices.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    if (min - max).abs() < 0.01 {
                        format!("{min:.2} {currency}")
                    } else {
                        format!("{min:.2} - {max:.2} {currency}")
                    }
                };

                json!({
                    "title": p.title,
                    "handle": p.handle,
                    "status": format!("{:?}", p.status),
                    "price": price_range,
                    "variant_count": p.variants.len(),
                    "total_inventory": p.total_inventory,
                    "vendor": p.vendor
                })
            })
            .collect();

        Ok(json!({
            "count": products.len(),
            "products": products
        })
        .to_string())
    }

    pub(super) async fn get_inventory_summary(
        &self,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        let low_stock_threshold = input["low_stock_threshold"].as_i64().unwrap_or(10);

        // Get all products with inventory
        let mut out_of_stock: Vec<serde_json::Value> = Vec::new();
        let mut low_stock: Vec<serde_json::Value> = Vec::new();
        let mut total_skus = 0i64;
        let mut total_inventory = 0i64;
        let mut cursor: Option<String> = None;

        loop {
            let result = self
                .shopify
                .get_products(50, cursor.clone(), None)
                .await
                .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get products: {e}")))?;

            for product in &result.products {
                for variant in &product.variants {
                    total_skus += 1;
                    let qty = variant.inventory_quantity;
                    total_inventory += qty;

                    let variant_info = json!({
                        "product": product.title,
                        "variant": variant.title,
                        "sku": variant.sku,
                        "quantity": qty
                    });

                    if qty <= 0 {
                        out_of_stock.push(variant_info);
                    } else if qty < low_stock_threshold {
                        low_stock.push(variant_info);
                    }
                }
            }

            if !result.page_info.has_next_page {
                break;
            }
            cursor = result.page_info.end_cursor;
        }

        // Sort by quantity ascending (most critical first)
        out_of_stock.sort_by(|a, b| {
            let a_qty = a
                .get("quantity")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            let b_qty = b
                .get("quantity")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            a_qty.cmp(&b_qty)
        });
        low_stock.sort_by(|a, b| {
            let a_qty = a
                .get("quantity")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            let b_qty = b
                .get("quantity")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            a_qty.cmp(&b_qty)
        });

        // Limit lists to top 20 items
        out_of_stock.truncate(20);
        low_stock.truncate(20);

        Ok(json!({
            "summary": {
                "total_skus": total_skus,
                "total_inventory_units": total_inventory,
                "out_of_stock_count": out_of_stock.len(),
                "low_stock_count": low_stock.len(),
                "low_stock_threshold": low_stock_threshold
            },
            "out_of_stock": out_of_stock,
            "low_stock": low_stock
        })
        .to_string())
    }

    // =========================================================================
    // Finance Tools
    // =========================================================================

    pub(super) async fn get_profit_summary(
        &self,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        let start_date = input["start_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: start_date".to_string())
        })?;
        let end_date = input["end_date"].as_str().ok_or_else(|| {
            ClaudeError::ToolExecution("Missing required field: end_date".to_string())
        })?;

        let query = format!("created_at:>={start_date} created_at:<={end_date}");

        let mut gross_sales = 0.0;
        let mut currency = String::from("USD");
        let mut cursor: Option<String> = None;

        loop {
            let result = self
                .shopify
                .get_orders_list(50, cursor.clone(), Some(query.clone()), None, false)
                .await
                .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get orders: {e}")))?;

            for order in &result.orders {
                currency.clone_from(&order.currency_code);
                for line_item in &order.line_items {
                    let unit_price = line_item
                        .original_unit_price
                        .amount
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    #[allow(clippy::cast_precision_loss)]
                    let line_total = unit_price * line_item.quantity as f64;
                    gross_sales += line_total;
                }
            }

            if !result.page_info.has_next_page {
                break;
            }
            cursor = result.page_info.end_cursor;
        }

        // Note: COGS calculation would require inventory item cost data
        // which isn't available in the basic order line item
        Ok(json!({
            "period": {
                "start_date": start_date,
                "end_date": end_date
            },
            "summary": {
                "gross_sales": format!("{gross_sales:.2} {currency}")
            },
            "note": "Cost of goods sold (COGS) requires inventory item cost data. Set 'Cost per item' on products in Shopify for profit calculations."
        })
        .to_string())
    }

    pub(super) async fn get_payout_summary(
        &self,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        let limit = input["limit"].as_i64().unwrap_or(5);

        // Get recent payouts
        let payouts_result = self
            .shopify
            .get_payouts(limit, None, None, true)
            .await
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get payouts: {e}")))?;

        let payouts: Vec<serde_json::Value> = payouts_result
            .payouts
            .iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "status": format!("{}", p.status),
                    "amount": format!("{} {}", p.net.amount, p.net.currency_code),
                    "date": p.issued_at
                })
            })
            .collect();

        // Get payout schedule
        let schedule = self.shopify.get_payout_schedule().await.ok();

        // Get open disputes
        let disputes_result = self.shopify.get_disputes(10, None, None).await.ok();

        let open_disputes: Vec<serde_json::Value> = disputes_result
            .map(|r| {
                r.disputes
                    .iter()
                    .filter(|d| !matches!(d.status, DisputeStatus::Won | DisputeStatus::Lost))
                    .map(|d| {
                        let reason = d
                            .reason_details
                            .as_ref()
                            .map_or("Unknown", |r| r.reason.as_str());
                        json!({
                            "id": d.id,
                            "status": format!("{}", d.status),
                            "amount": format!("{} {}", d.amount.amount, d.amount.currency_code),
                            "reason": reason
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(json!({
            "recent_payouts": payouts,
            "payout_schedule": schedule.map(|s| json!({
                "interval": format!("{}", s.interval),
                "monthly_anchor": s.monthly_anchor,
                "weekly_anchor": s.weekly_anchor
            })),
            "open_disputes": {
                "count": open_disputes.len(),
                "disputes": open_disputes
            }
        })
        .to_string())
    }

    pub(super) async fn get_gift_card_summary(
        &self,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        let start_date = input["start_date"].as_str();
        let end_date = input["end_date"].as_str();

        // Get total gift cards count
        let total_count = self.shopify.get_gift_cards_count(None).await.map_err(|e| {
            ClaudeError::ToolExecution(format!("Failed to get gift card count: {e}"))
        })?;

        // Get enabled gift cards to calculate outstanding balance
        let enabled_result = self
            .shopify
            .get_gift_cards(100, None, Some("enabled:true".to_string()), None, false)
            .await
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get gift cards: {e}")))?;

        let mut outstanding_balance = 0.0;
        let mut currency = String::from("USD");

        for gc in &enabled_result.gift_cards {
            outstanding_balance += gc.balance.amount.parse::<f64>().unwrap_or(0.0);
            currency.clone_from(&gc.balance.currency_code);
        }

        // Get disabled count
        let disabled_count = self
            .shopify
            .get_gift_cards_count(Some("enabled:false".to_string()))
            .await
            .unwrap_or(0);

        // Get gift cards sold in period if dates provided
        let sold_in_period = if let (Some(start), Some(end)) = (start_date, end_date) {
            let period_query = format!("created_at:>={start} created_at:<={end}");
            Some(
                self.shopify
                    .get_gift_cards_count(Some(period_query))
                    .await
                    .unwrap_or(0),
            )
        } else {
            None
        };

        let mut response = json!({
            "summary": {
                "total_gift_cards": total_count,
                "outstanding_balance": format!("{outstanding_balance:.2} {currency}"),
                "disabled_count": disabled_count
            }
        });

        if let (Some(sold), Some(obj)) = (sold_in_period, response.as_object_mut()) {
            obj.insert(
                "period".to_string(),
                json!({
                    "start_date": start_date,
                    "end_date": end_date,
                    "gift_cards_sold": sold
                }),
            );
        }

        Ok(response.to_string())
    }

    // =========================================================================
    // Fulfillment Tools
    // =========================================================================

    pub(super) async fn get_fulfillment_summary(
        &self,
        _input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        // Get unfulfilled orders
        let unfulfilled = self
            .shopify
            .get_orders(50, None, Some("fulfillment_status:unfulfilled".to_string()))
            .await
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get orders: {e}")))?;

        // Get partially fulfilled orders
        let partial = self
            .shopify
            .get_orders(50, None, Some("fulfillment_status:partial".to_string()))
            .await
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get orders: {e}")))?;

        // Get orders fulfilled today
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let fulfilled_today = self
            .shopify
            .get_orders(
                50,
                None,
                Some(format!("fulfillment_status:fulfilled updated_at:>={today}")),
            )
            .await
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get orders: {e}")))?;

        // Build list of awaiting fulfillment with order names
        let awaiting: Vec<serde_json::Value> = unfulfilled
            .orders
            .iter()
            .take(10)
            .map(|o| {
                json!({
                    "order": o.name,
                    "created_at": o.created_at,
                    "items": o.line_items.len()
                })
            })
            .collect();

        Ok(json!({
            "summary": {
                "awaiting_fulfillment": unfulfilled.orders.len(),
                "partially_fulfilled": partial.orders.len(),
                "fulfilled_today": fulfilled_today.orders.len()
            },
            "awaiting_fulfillment_orders": awaiting
        })
        .to_string())
    }
}

//! Shopify order lookups for email triage context.
//!
//! Provides order data to the response composer so draft replies can
//! reference real order status, fulfillment tracking, and line items.

use serde_json::json;
use tracing::{debug, instrument};

use super::client::{ShopifyClient, ShopifyError};

/// Summary of a Shopify order for inclusion in response prompts.
#[derive(Debug)]
pub struct OrderSummary {
    /// Order display name (e.g., "#1234").
    pub name: String,
    /// Financial status (e.g., "paid", "refunded").
    pub financial_status: String,
    /// Fulfillment status (e.g., "fulfilled", "unfulfilled").
    pub fulfillment_status: String,
    /// Order creation date.
    pub created_at: String,
    /// Line item descriptions.
    pub line_items: Vec<String>,
    /// Tracking numbers (if fulfilled).
    pub tracking_numbers: Vec<String>,
    /// Tracking URLs (if fulfilled).
    pub tracking_urls: Vec<String>,
}

const ORDER_LOOKUP_QUERY: &str = r"
query OrderLookup($query: String!) {
    orders(first: 3, query: $query) {
        nodes {
            name
            displayFinancialStatus
            displayFulfillmentStatus
            createdAt
            lineItems(first: 10) {
                nodes {
                    title
                    quantity
                    variant { title }
                }
            }
            fulfillments {
                trackingInfo {
                    number
                    url
                }
            }
        }
    }
}
";

/// Look up orders by order number (e.g., "#1234" or "1234").
#[instrument(skip(client))]
pub async fn lookup_by_number(
    client: &ShopifyClient,
    order_number: &str,
) -> Result<Vec<OrderSummary>, ShopifyError> {
    let clean_number = order_number.trim_start_matches('#');
    let query_str = format!("name:{clean_number}");

    debug!(query = %query_str, "looking up order by number");

    let data = client
        .graphql(ORDER_LOOKUP_QUERY, json!({ "query": query_str }))
        .await?;

    Ok(parse_orders(&data))
}

/// Look up recent orders by customer email address.
#[instrument(skip(client))]
pub async fn lookup_by_email(
    client: &ShopifyClient,
    email: &str,
) -> Result<Vec<OrderSummary>, ShopifyError> {
    let query_str = format!("email:{email}");

    debug!(query = %query_str, "looking up orders by email");

    let data = client
        .graphql(ORDER_LOOKUP_QUERY, json!({ "query": query_str }))
        .await?;

    Ok(parse_orders(&data))
}

/// Parse order nodes from the GraphQL response.
fn parse_orders(data: &serde_json::Value) -> Vec<OrderSummary> {
    let Some(nodes) = data
        .get("orders")
        .and_then(|o| o.get("nodes"))
        .and_then(|n| n.as_array())
    else {
        return Vec::new();
    };

    nodes.iter().filter_map(parse_single_order).collect()
}

/// Parse a single order node.
fn parse_single_order(node: &serde_json::Value) -> Option<OrderSummary> {
    let name = node.get("name")?.as_str()?.to_string();

    let financial_status = node
        .get("displayFinancialStatus")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let fulfillment_status = node
        .get("displayFulfillmentStatus")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let created_at = node
        .get("createdAt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let line_items = node
        .get("lineItems")
        .and_then(|li| li.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let title = item.get("title")?.as_str()?;
                    let qty = item
                        .get("quantity")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(1);
                    let variant = item
                        .get("variant")
                        .and_then(|v| v.get("title"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    if variant.is_empty() || variant == "Default Title" {
                        Some(format!("{title} x{qty}"))
                    } else {
                        Some(format!("{title} ({variant}) x{qty}"))
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let (tracking_numbers, tracking_urls) = parse_tracking_info(node);

    Some(OrderSummary {
        name,
        financial_status,
        fulfillment_status,
        created_at,
        line_items,
        tracking_numbers,
        tracking_urls,
    })
}

/// Extract tracking info from fulfillments.
fn parse_tracking_info(node: &serde_json::Value) -> (Vec<String>, Vec<String>) {
    let mut numbers = Vec::new();
    let mut urls = Vec::new();

    let Some(fulfillments) = node.get("fulfillments").and_then(|f| f.as_array()) else {
        return (numbers, urls);
    };

    for fulfillment in fulfillments {
        let Some(tracking) = fulfillment.get("trackingInfo").and_then(|t| t.as_array()) else {
            continue;
        };
        for info in tracking {
            if let Some(num) = info.get("number").and_then(|n| n.as_str()) {
                numbers.push(num.to_string());
            }
            if let Some(url) = info.get("url").and_then(|u| u.as_str()) {
                urls.push(url.to_string());
            }
        }
    }

    (numbers, urls)
}

/// Format order summaries as context text for the response prompt.
#[must_use]
pub fn format_orders_for_prompt(orders: &[OrderSummary]) -> String {
    use std::fmt::Write;

    if orders.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(1000);
    out.push_str("Shopify Order Data:\n");

    for order in orders {
        let _ = writeln!(out, "- Order {}: placed {}", order.name, order.created_at);
        let _ = writeln!(
            out,
            "  Payment: {}, Fulfillment: {}",
            order.financial_status, order.fulfillment_status
        );
        if !order.line_items.is_empty() {
            let _ = writeln!(out, "  Items: {}", order.line_items.join(", "));
        }
        if !order.tracking_numbers.is_empty() {
            let _ = writeln!(out, "  Tracking: {}", order.tracking_numbers.join(", "));
        }
        if !order.tracking_urls.is_empty() {
            let _ = writeln!(out, "  Tracking URLs: {}", order.tracking_urls.join(", "));
        }
    }

    out
}

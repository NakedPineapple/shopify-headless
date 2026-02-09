//! Shopify order and fulfillment queries for outbound email triggers.
//!
//! Polls the Shopify Admin GraphQL API for recent orders and fulfillments
//! to determine which transactional emails need to be sent.

use serde_json::json;
use tracing::{debug, instrument};

use super::client::{ShopifyClient, ShopifyError};

/// An order with enriched data for rendering transactional emails.
#[derive(Debug)]
pub struct OrderDetail {
    /// Shopify global ID (e.g. `gid://shopify/Order/123456`).
    pub id: String,
    /// Display name (e.g. "#1234").
    pub name: String,
    /// Customer email address.
    pub email: Option<String>,
    /// Customer first name.
    pub customer_first_name: Option<String>,
    /// Customer last name.
    pub customer_last_name: Option<String>,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// Line items in the order.
    pub line_items: Vec<OrderLineItem>,
    /// Price summary.
    pub prices: OrderPrices,
    /// Shipping address.
    pub shipping_address: Option<ShippingAddress>,
    /// Fulfillment details.
    pub fulfillments: Vec<FulfillmentDetail>,
}

/// A line item in an order.
#[derive(Debug)]
pub struct OrderLineItem {
    pub title: String,
    pub variant_title: Option<String>,
    pub quantity: i64,
    pub price: String,
}

/// Price breakdown for an order.
#[derive(Debug)]
pub struct OrderPrices {
    pub subtotal: String,
    pub shipping: String,
    pub tax: String,
    pub total: String,
}

/// A shipping address from an order.
#[derive(Debug)]
pub struct ShippingAddress {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub address1: Option<String>,
    pub address2: Option<String>,
    pub city: Option<String>,
    pub province_code: Option<String>,
    pub zip: Option<String>,
    pub country: Option<String>,
}

/// Fulfillment details from an order.
#[derive(Debug)]
pub struct FulfillmentDetail {
    pub number: Option<String>,
    pub url: Option<String>,
    pub company: Option<String>,
}

const ORDER_BY_ID_QUERY: &str = r"
query OrderById($id: ID!) {
    order(id: $id) {
        id
        name
        email
        createdAt
        customer { firstName lastName }
        lineItems(first: 20) {
            nodes {
                title
                quantity
                variant { title }
                originalUnitPriceSet { shopMoney { amount currencyCode } }
            }
        }
        subtotalPriceSet { shopMoney { amount currencyCode } }
        totalPriceSet { shopMoney { amount currencyCode } }
        totalShippingPriceSet { shopMoney { amount currencyCode } }
        totalTaxSet { shopMoney { amount currencyCode } }
        shippingAddress {
            firstName lastName
            address1 address2
            city provinceCode zip country
        }
        fulfillments {
            status
            createdAt
            trackingInfo { number url company }
        }
    }
}
";

const RECENT_ORDERS_QUERY: &str = r"
query RecentOrders($query: String!) {
    orders(first: 50, query: $query, sortKey: CREATED_AT, reverse: true) {
        nodes {
            id
            name
            email
            createdAt
            customer { firstName lastName }
            displayFinancialStatus
            displayFulfillmentStatus
            lineItems(first: 20) {
                nodes {
                    title
                    quantity
                    variant { title }
                    originalUnitPriceSet { shopMoney { amount currencyCode } }
                }
            }
            subtotalPriceSet { shopMoney { amount currencyCode } }
            totalPriceSet { shopMoney { amount currencyCode } }
            totalShippingPriceSet { shopMoney { amount currencyCode } }
            totalTaxSet { shopMoney { amount currencyCode } }
            shippingAddress {
                firstName lastName
                address1 address2
                city provinceCode zip country
            }
            fulfillments {
                status
                createdAt
                trackingInfo { number url company }
            }
        }
    }
}
";

/// Fetch recent orders created within the last `minutes` minutes.
#[instrument(skip(client))]
pub async fn fetch_recent_orders(
    client: &ShopifyClient,
    minutes: u64,
) -> Result<Vec<OrderDetail>, ShopifyError> {
    let since =
        chrono::Utc::now() - chrono::Duration::minutes(i64::try_from(minutes).unwrap_or(10));
    let query_str = format!("created_at:>'{}'", since.format("%Y-%m-%dT%H:%M:%S%z"));

    debug!(query = %query_str, "fetching recent orders");

    let data = client
        .graphql(RECENT_ORDERS_QUERY, json!({ "query": query_str }))
        .await?;

    Ok(parse_order_details(&data))
}

/// Fetch recently fulfilled orders (updated within the last `minutes` minutes).
#[instrument(skip(client))]
pub async fn fetch_recently_fulfilled(
    client: &ShopifyClient,
    minutes: u64,
) -> Result<Vec<OrderDetail>, ShopifyError> {
    let since =
        chrono::Utc::now() - chrono::Duration::minutes(i64::try_from(minutes).unwrap_or(10));
    let query_str = format!(
        "fulfillment_status:shipped updated_at:>'{}'",
        since.format("%Y-%m-%dT%H:%M:%S%z")
    );

    debug!(query = %query_str, "fetching recently fulfilled orders");

    let data = client
        .graphql(RECENT_ORDERS_QUERY, json!({ "query": query_str }))
        .await?;

    Ok(parse_order_details(&data))
}

/// Fetch recently delivered orders.
#[instrument(skip(client))]
pub async fn fetch_recently_delivered(
    client: &ShopifyClient,
    minutes: u64,
) -> Result<Vec<OrderDetail>, ShopifyError> {
    let since =
        chrono::Utc::now() - chrono::Duration::minutes(i64::try_from(minutes).unwrap_or(60));
    let query_str = format!(
        "fulfillment_status:delivered updated_at:>'{}'",
        since.format("%Y-%m-%dT%H:%M:%S%z")
    );

    debug!(query = %query_str, "fetching recently delivered orders");

    let data = client
        .graphql(RECENT_ORDERS_QUERY, json!({ "query": query_str }))
        .await?;

    Ok(parse_order_details(&data))
}

/// Fetch a single order by its Shopify global ID.
///
/// Used by the webhook event processor to fetch full order details after
/// receiving a webhook notification.
#[instrument(skip(client))]
pub async fn fetch_order_by_id(
    client: &ShopifyClient,
    order_gid: &str,
) -> Result<Option<OrderDetail>, ShopifyError> {
    let data = client
        .graphql(ORDER_BY_ID_QUERY, json!({ "id": order_gid }))
        .await?;

    let order_node = data.get("order");
    match order_node {
        Some(node) if !node.is_null() => Ok(parse_single_detail(node)),
        _ => Ok(None),
    }
}

fn parse_order_details(data: &serde_json::Value) -> Vec<OrderDetail> {
    let Some(nodes) = data
        .get("orders")
        .and_then(|o| o.get("nodes"))
        .and_then(|n| n.as_array())
    else {
        return Vec::new();
    };

    nodes.iter().filter_map(parse_single_detail).collect()
}

fn parse_single_detail(node: &serde_json::Value) -> Option<OrderDetail> {
    let id = node.get("id")?.as_str()?.to_string();
    let name = node.get("name")?.as_str()?.to_string();
    let email = node.get("email").and_then(|v| v.as_str()).map(String::from);
    let created_at = str_field(node, "createdAt");

    let customer_first_name = node
        .get("customer")
        .and_then(|c| c.get("firstName"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let customer_last_name = node
        .get("customer")
        .and_then(|c| c.get("lastName"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let line_items = parse_line_items(node);
    let prices = parse_prices(node);
    let shipping_address = parse_shipping_address(node);
    let fulfillments = parse_fulfillments(node);

    Some(OrderDetail {
        id,
        name,
        email,
        customer_first_name,
        customer_last_name,
        created_at,
        line_items,
        prices,
        shipping_address,
        fulfillments,
    })
}

fn str_field(node: &serde_json::Value, key: &str) -> String {
    node.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn money_field(node: &serde_json::Value, key: &str) -> String {
    node.get(key)
        .and_then(|v| v.get("shopMoney"))
        .and_then(|m| m.get("amount"))
        .and_then(|v| v.as_str())
        .unwrap_or("0.00")
        .to_string()
}

fn parse_line_items(node: &serde_json::Value) -> Vec<OrderLineItem> {
    let Some(items) = node
        .get("lineItems")
        .and_then(|li| li.get("nodes"))
        .and_then(|n| n.as_array())
    else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| {
            let title = item.get("title")?.as_str()?.to_string();
            let variant_title = item
                .get("variant")
                .and_then(|v| v.get("title"))
                .and_then(|t| t.as_str())
                .filter(|t| *t != "Default Title")
                .map(String::from);
            let quantity = item
                .get("quantity")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(1);
            let price = money_field(item, "originalUnitPriceSet");
            Some(OrderLineItem {
                title,
                variant_title,
                quantity,
                price,
            })
        })
        .collect()
}

fn parse_prices(node: &serde_json::Value) -> OrderPrices {
    let subtotal = money_field(node, "subtotalPriceSet");
    let shipping = money_field(node, "totalShippingPriceSet");
    let tax = money_field(node, "totalTaxSet");
    let total = money_field(node, "totalPriceSet");
    OrderPrices {
        subtotal,
        shipping,
        tax,
        total,
    }
}

fn parse_shipping_address(node: &serde_json::Value) -> Option<ShippingAddress> {
    let addr = node.get("shippingAddress")?;
    if addr.is_null() {
        return None;
    }
    Some(ShippingAddress {
        first_name: addr
            .get("firstName")
            .and_then(|v| v.as_str())
            .map(String::from),
        last_name: addr
            .get("lastName")
            .and_then(|v| v.as_str())
            .map(String::from),
        address1: addr
            .get("address1")
            .and_then(|v| v.as_str())
            .map(String::from),
        address2: addr
            .get("address2")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from),
        city: addr.get("city").and_then(|v| v.as_str()).map(String::from),
        province_code: addr
            .get("provinceCode")
            .and_then(|v| v.as_str())
            .map(String::from),
        zip: addr.get("zip").and_then(|v| v.as_str()).map(String::from),
        country: addr
            .get("country")
            .and_then(|v| v.as_str())
            .map(String::from),
    })
}

fn parse_fulfillments(node: &serde_json::Value) -> Vec<FulfillmentDetail> {
    let Some(fulfillments) = node.get("fulfillments").and_then(|f| f.as_array()) else {
        return Vec::new();
    };

    fulfillments
        .iter()
        .map(|f| {
            let tracking = f
                .get("trackingInfo")
                .and_then(|t| t.as_array())
                .and_then(|a| a.first());
            let number = tracking
                .and_then(|t| t.get("number"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let url = tracking
                .and_then(|t| t.get("url"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let company = tracking
                .and_then(|t| t.get("company"))
                .and_then(|v| v.as_str())
                .map(String::from);
            FulfillmentDetail {
                number,
                url,
                company,
            }
        })
        .collect()
}

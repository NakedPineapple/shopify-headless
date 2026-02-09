//! Shopify abandoned checkout queries for cart recovery workflows.
//!
//! Queries the Shopify Admin GraphQL API for abandoned checkouts and
//! checks for recovered orders (completed purchases by the same customer).

use serde_json::json;
use tracing::{debug, instrument};

use super::client::{ShopifyClient, ShopifyError};

/// An abandoned checkout from Shopify.
#[derive(Debug)]
pub struct AbandonedCheckout {
    /// Shopify global ID (e.g., `gid://shopify/AbandonedCheckout/123`).
    pub id: String,
    /// Customer email address.
    pub email: Option<String>,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// ISO 8601 last update timestamp.
    pub updated_at: String,
    /// Cart total amount as a string (e.g., "49.99").
    pub total: String,
    /// Line items in the abandoned cart.
    pub line_items: Vec<CheckoutLineItem>,
}

/// A line item in an abandoned checkout.
#[derive(Debug)]
pub struct CheckoutLineItem {
    /// Product title.
    pub title: String,
    /// Quantity.
    pub quantity: i64,
    /// Variant title (if applicable).
    pub variant_title: Option<String>,
}

const ABANDONED_CHECKOUTS_QUERY: &str = r"
query AbandonedCheckouts($query: String!) {
    abandonedCheckouts(first: 50, query: $query, sortKey: UPDATED_AT, reverse: true) {
        nodes {
            id
            createdAt
            updatedAt
            email
            abandonedCheckoutUrl
            totalPriceSet { shopMoney { amount currencyCode } }
            lineItems(first: 20) {
                nodes {
                    title
                    quantity
                    variant { title }
                }
            }
        }
    }
}
";

const ORDERS_BY_EMAIL_QUERY: &str = r"
query OrdersByEmail($query: String!) {
    orders(first: 1, query: $query, sortKey: CREATED_AT, reverse: true) {
        nodes {
            id
            name
        }
    }
}
";

/// Fetch abandoned checkouts updated within the last `minutes` minutes.
#[instrument(skip(client))]
pub async fn fetch_abandoned_checkouts(
    client: &ShopifyClient,
    minutes: u64,
) -> Result<Vec<AbandonedCheckout>, ShopifyError> {
    let since =
        chrono::Utc::now() - chrono::Duration::minutes(i64::try_from(minutes).unwrap_or(60));
    let query_str = format!("updated_at:>'{}'", since.format("%Y-%m-%dT%H:%M:%S%z"));

    debug!(query = %query_str, "fetching abandoned checkouts");

    let data = client
        .graphql(ABANDONED_CHECKOUTS_QUERY, json!({ "query": query_str }))
        .await?;

    Ok(parse_abandoned_checkouts(&data))
}

/// Check if a customer email has any orders created after the given ISO 8601 timestamp.
///
/// Returns the Shopify order ID if found, or `None` if no matching order exists.
#[instrument(skip(client))]
pub async fn find_recovery_order(
    client: &ShopifyClient,
    email: &str,
    abandoned_at: &str,
) -> Result<Option<String>, ShopifyError> {
    let query_str = format!("email:'{email}' created_at:>'{abandoned_at}'");

    debug!(query = %query_str, "checking for recovery order");

    let data = client
        .graphql(ORDERS_BY_EMAIL_QUERY, json!({ "query": query_str }))
        .await?;

    let order_id = data
        .get("orders")
        .and_then(|o| o.get("nodes"))
        .and_then(|n| n.as_array())
        .and_then(|arr| arr.first())
        .and_then(|node| node.get("id"))
        .and_then(|v| v.as_str())
        .map(String::from);

    Ok(order_id)
}

fn parse_abandoned_checkouts(data: &serde_json::Value) -> Vec<AbandonedCheckout> {
    let Some(nodes) = data
        .get("abandonedCheckouts")
        .and_then(|o| o.get("nodes"))
        .and_then(|n| n.as_array())
    else {
        return Vec::new();
    };

    nodes.iter().filter_map(parse_single_checkout).collect()
}

fn parse_single_checkout(node: &serde_json::Value) -> Option<AbandonedCheckout> {
    let id = node.get("id")?.as_str()?.to_string();
    let email = node
        .get("email")
        .and_then(|v| v.as_str())
        .filter(|e| !e.is_empty())
        .map(String::from);
    let created_at = node
        .get("createdAt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let updated_at = node
        .get("updatedAt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let total = node
        .get("totalPriceSet")
        .and_then(|v| v.get("shopMoney"))
        .and_then(|m| m.get("amount"))
        .and_then(|v| v.as_str())
        .unwrap_or("0.00")
        .to_string();

    let line_items = parse_checkout_line_items(node);

    Some(AbandonedCheckout {
        id,
        email,
        created_at,
        updated_at,
        total,
        line_items,
    })
}

fn parse_checkout_line_items(node: &serde_json::Value) -> Vec<CheckoutLineItem> {
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
            let quantity = item
                .get("quantity")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(1);
            let variant_title = item
                .get("variant")
                .and_then(|v| v.get("title"))
                .and_then(|t| t.as_str())
                .filter(|t| *t != "Default Title")
                .map(String::from);
            Some(CheckoutLineItem {
                title,
                quantity,
                variant_title,
            })
        })
        .collect()
}

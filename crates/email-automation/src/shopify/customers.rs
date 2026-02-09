//! Shopify customer queries for segmentation workflows.
//!
//! Fetches customers with their order history so the segmentation engine
//! can classify them into segments and apply appropriate tags.

use serde_json::json;
use tracing::{debug, instrument, warn};

use super::client::{ShopifyClient, ShopifyError};

/// A customer with order history data for segmentation.
#[derive(Debug)]
pub struct CustomerProfile {
    /// Shopify customer global ID.
    pub id: String,
    /// Customer email address.
    pub email: Option<String>,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Total number of orders.
    pub order_count: i32,
    /// Lifetime spend amount (as decimal string, e.g., "499.99").
    pub amount_spent: String,
    /// Current tags on the customer.
    pub tags: Vec<String>,
    /// Date of last order (ISO 8601).
    pub last_order_at: Option<String>,
}

const CUSTOMER_QUERY: &str = r"
query Customers($first: Int!, $after: String, $query: String) {
    customers(first: $first, after: $after, query: $query) {
        nodes {
            id
            email
            firstName
            lastName
            numberOfOrders
            amountSpent {
                amount
                currencyCode
            }
            tags
            lastOrder {
                createdAt
            }
            createdAt
        }
        pageInfo {
            hasNextPage
            endCursor
        }
    }
}
";

const TAG_ADD_MUTATION: &str = r"
mutation TagsAdd($id: ID!, $tags: [String!]!) {
    tagsAdd(id: $id, tags: $tags) {
        node { id }
        userErrors { field message }
    }
}
";

const TAG_REMOVE_MUTATION: &str = r"
mutation TagsRemove($id: ID!, $tags: [String!]!) {
    tagsRemove(id: $id, tags: $tags) {
        node { id }
        userErrors { field message }
    }
}
";

/// Fetch all enabled customers with their order history.
///
/// Paginates through all customers in batches of 50.
#[instrument(skip(client))]
pub async fn fetch_all_customers(
    client: &ShopifyClient,
) -> Result<Vec<CustomerProfile>, ShopifyError> {
    let mut all_customers = Vec::new();
    let mut cursor: Option<String> = None;
    let page_size = 50;

    loop {
        let variables = cursor.as_ref().map_or_else(
            || json!({ "first": page_size, "query": "state:enabled" }),
            |c| json!({ "first": page_size, "after": c, "query": "state:enabled" }),
        );

        let data = client.graphql(CUSTOMER_QUERY, variables).await?;

        let customers_data = data.get("customers");
        let nodes = customers_data
            .and_then(|c| c.get("nodes"))
            .and_then(|n| n.as_array());

        let Some(nodes) = nodes else {
            warn!("no customers found in response");
            break;
        };

        for node in nodes {
            if let Some(customer) = parse_customer(node) {
                all_customers.push(customer);
            }
        }

        let has_next = customers_data
            .and_then(|c| c.get("pageInfo"))
            .and_then(|pi| pi.get("hasNextPage"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        if !has_next {
            break;
        }

        cursor = customers_data
            .and_then(|c| c.get("pageInfo"))
            .and_then(|pi| pi.get("endCursor"))
            .and_then(|c| c.as_str())
            .map(String::from);
    }

    debug!(count = all_customers.len(), "fetched customers");
    Ok(all_customers)
}

/// Apply segment tags to a customer, removing any stale segment tags.
///
/// This ensures only one `segment:*` tag is active at a time.
#[instrument(skip(client), fields(customer_id = %customer_id, segment = %new_tag))]
pub async fn apply_segment_tag(
    client: &ShopifyClient,
    customer_id: &str,
    current_tags: &[String],
    new_tag: &str,
) -> Result<(), ShopifyError> {
    // Determine which existing segment tags to remove
    let tags_to_remove: Vec<String> = current_tags
        .iter()
        .filter(|t| t.starts_with("segment:") && t.as_str() != new_tag)
        .cloned()
        .collect();

    // Remove stale segment tags
    if !tags_to_remove.is_empty() {
        let variables = json!({
            "id": customer_id,
            "tags": tags_to_remove,
        });

        let data = client.graphql(TAG_REMOVE_MUTATION, variables).await?;
        check_user_errors(&data, "tagsRemove")?;
    }

    // Add the new segment tag if not already present
    if !current_tags.iter().any(|t| t == new_tag) {
        let variables = json!({
            "id": customer_id,
            "tags": [new_tag],
        });

        let data = client.graphql(TAG_ADD_MUTATION, variables).await?;
        check_user_errors(&data, "tagsAdd")?;
    }

    debug!("segment tag applied");
    Ok(())
}

/// Check for `userErrors` in a Shopify mutation response.
fn check_user_errors(data: &serde_json::Value, mutation: &str) -> Result<(), ShopifyError> {
    let errors = data
        .get(mutation)
        .and_then(|m| m.get("userErrors"))
        .and_then(|e| e.as_array());

    if let Some(errors) = errors {
        let messages: Vec<&str> = errors
            .iter()
            .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
            .collect();
        if !messages.is_empty() {
            return Err(ShopifyError::GraphQL(messages.join("; ")));
        }
    }

    Ok(())
}

/// Parse a single customer node into a `CustomerProfile`.
fn parse_customer(node: &serde_json::Value) -> Option<CustomerProfile> {
    let id = node.get("id")?.as_str()?.to_string();

    let email = node
        .get("email")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let first_name = node
        .get("firstName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let last_name = node
        .get("lastName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let order_count = node
        .get("numberOfOrders")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    let order_count = i32::try_from(order_count).unwrap_or(0);

    let amount_spent = node
        .get("amountSpent")
        .and_then(|a| a.get("amount"))
        .and_then(|v| v.as_str())
        .unwrap_or("0.00")
        .to_string();

    let tags = node
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let last_order_at = node
        .get("lastOrder")
        .and_then(|lo| lo.get("createdAt"))
        .and_then(|v| v.as_str())
        .map(String::from);

    Some(CustomerProfile {
        id,
        email,
        first_name,
        last_name,
        order_count,
        amount_spent,
        tags,
        last_order_at,
    })
}

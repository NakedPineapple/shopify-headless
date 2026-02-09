//! Shopify subscription contract queries for lifecycle workflows.
//!
//! Queries the Shopify Admin API for subscription contracts and their billing
//! attempts to detect upcoming renewals, payment failures, and cancellations.

use serde_json::json;
use tracing::{debug, instrument};

use super::client::{ShopifyClient, ShopifyError};

/// Summary of a Shopify subscription contract.
#[derive(Debug)]
pub struct SubscriptionContract {
    /// Shopify global ID (e.g., `gid://shopify/SubscriptionContract/123`).
    pub id: String,
    /// Customer email.
    pub customer_email: Option<String>,
    /// Customer first name.
    pub customer_first_name: Option<String>,
    /// Customer last name.
    pub customer_last_name: Option<String>,
    /// Next billing date (ISO 8601).
    pub next_billing_date: Option<String>,
    /// Product line items in the subscription.
    pub line_items: Vec<String>,
}

const ACTIVE_CONTRACTS_QUERY: &str = r#"
query ActiveContracts($first: Int!, $after: String) {
    subscriptionContracts(first: $first, after: $after, query: "status:ACTIVE") {
        pageInfo { hasNextPage endCursor }
        nodes {
            id
            status
            nextBillingDate
            customer {
                email
                firstName
                lastName
            }
            lines(first: 5) {
                nodes {
                    title
                    variantTitle
                    quantity
                }
            }
            lastPaymentStatus
            orders(first: 1, reverse: true) {
                nodes { createdAt }
            }
        }
    }
}
"#;

const CANCELLED_CONTRACTS_QUERY: &str = r"
query CancelledContracts($first: Int!, $query: String!) {
    subscriptionContracts(first: $first, query: $query) {
        nodes {
            id
            status
            customer {
                email
                firstName
                lastName
            }
            lines(first: 5) {
                nodes {
                    title
                    variantTitle
                    quantity
                }
            }
            orders(first: 1, reverse: true) {
                nodes { createdAt }
            }
        }
    }
}
";

const BILLING_ATTEMPTS_QUERY: &str = r"
query RecentBillingAttempts($contractId: ID!) {
    subscriptionBillingAttempts(first: 5, subscriptionContractId: $contractId, reverse: true) {
        nodes {
            id
            createdAt
            errorMessage
            ready
            subscriptionContract { id }
        }
    }
}
";

/// Fetch all active subscription contracts (paginated).
#[instrument(skip(client))]
pub async fn fetch_active_contracts(
    client: &ShopifyClient,
) -> Result<Vec<SubscriptionContract>, ShopifyError> {
    let mut contracts = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let variables = json!({
            "first": 50,
            "after": cursor,
        });

        let data = client.graphql(ACTIVE_CONTRACTS_QUERY, variables).await?;

        let sc = data
            .get("subscriptionContracts")
            .ok_or_else(|| ShopifyError::GraphQL("missing subscriptionContracts".to_string()))?;

        if let Some(nodes) = sc.get("nodes").and_then(|n| n.as_array()) {
            for node in nodes {
                if let Some(contract) = parse_contract(node) {
                    contracts.push(contract);
                }
            }
        }

        let has_next = sc
            .get("pageInfo")
            .and_then(|p| p.get("hasNextPage"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        if !has_next {
            break;
        }

        cursor = sc
            .get("pageInfo")
            .and_then(|p| p.get("endCursor"))
            .and_then(|c| c.as_str())
            .map(String::from);
    }

    debug!(
        count = contracts.len(),
        "fetched active subscription contracts"
    );
    Ok(contracts)
}

/// Fetch recently cancelled subscription contracts.
#[instrument(skip(client))]
pub async fn fetch_cancelled_contracts(
    client: &ShopifyClient,
    since_minutes: u64,
) -> Result<Vec<SubscriptionContract>, ShopifyError> {
    let since = chrono::Utc::now()
        - chrono::Duration::minutes(i64::try_from(since_minutes).unwrap_or(1440));
    let query_filter = format!("status:CANCELLED AND updated_at:>{}", since.to_rfc3339());

    let variables = json!({
        "first": 50,
        "query": query_filter,
    });

    let data = client.graphql(CANCELLED_CONTRACTS_QUERY, variables).await?;

    let nodes = data
        .get("subscriptionContracts")
        .and_then(|sc| sc.get("nodes"))
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();

    let contracts: Vec<SubscriptionContract> = nodes.iter().filter_map(parse_contract).collect();

    debug!(count = contracts.len(), "fetched cancelled contracts");
    Ok(contracts)
}

/// Check if a contract has recent failed billing attempts.
#[instrument(skip(client))]
pub async fn has_recent_billing_failure(
    client: &ShopifyClient,
    contract_id: &str,
) -> Result<Option<String>, ShopifyError> {
    let variables = json!({ "contractId": contract_id });
    let data = client.graphql(BILLING_ATTEMPTS_QUERY, variables).await?;

    let attempts = data
        .get("subscriptionBillingAttempts")
        .and_then(|ba| ba.get("nodes"))
        .and_then(|n| n.as_array());

    let Some(attempts) = attempts else {
        return Ok(None);
    };

    // Return the error message from the most recent failed attempt
    for attempt in attempts {
        if let Some(error_msg) = attempt
            .get("errorMessage")
            .and_then(|e| e.as_str())
            .filter(|e| !e.is_empty())
        {
            return Ok(Some(error_msg.to_string()));
        }
    }

    Ok(None)
}

/// Parse a subscription contract node from the GraphQL response.
fn parse_contract(node: &serde_json::Value) -> Option<SubscriptionContract> {
    let id = node.get("id")?.as_str()?.to_string();

    let customer = node.get("customer");
    let customer_email = customer
        .and_then(|c| c.get("email"))
        .and_then(|e| e.as_str())
        .map(String::from);
    let customer_first_name = customer
        .and_then(|c| c.get("firstName"))
        .and_then(|f| f.as_str())
        .map(String::from);
    let customer_last_name = customer
        .and_then(|c| c.get("lastName"))
        .and_then(|l| l.as_str())
        .map(String::from);

    let next_billing_date = node
        .get("nextBillingDate")
        .and_then(|d| d.as_str())
        .map(String::from);

    let line_items = node
        .get("lines")
        .and_then(|l| l.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let title = item.get("title")?.as_str()?;
                    let variant = item
                        .get("variantTitle")
                        .and_then(|v| v.as_str())
                        .filter(|v| !v.is_empty());
                    let qty = item
                        .get("quantity")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(1);
                    variant.map_or_else(
                        || Some(format!("{title} x{qty}")),
                        |v| Some(format!("{title} ({v}) x{qty}")),
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    Some(SubscriptionContract {
        id,
        customer_email,
        customer_first_name,
        customer_last_name,
        next_billing_date,
        line_items,
    })
}

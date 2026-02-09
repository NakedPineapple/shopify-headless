//! Shopify webhook subscription management.
//!
//! Programmatically registers webhook subscriptions via the Shopify Admin
//! GraphQL API on startup. Ensures subscriptions point to the correct
//! callback URL and cover the desired event topics.

use tracing::{info, warn};

use super::client::{ShopifyClient, ShopifyError};

/// Topics to subscribe to. These match the events the existing poller handles.
const DESIRED_TOPICS: &[&str] = &[
    "ORDERS_CREATE",
    "FULFILLMENTS_CREATE",
    "FULFILLMENTS_UPDATE",
];

/// Ensure webhook subscriptions exist for all desired topics.
///
/// Lists existing subscriptions, creates missing ones, and updates any
/// that point to the wrong callback URL. Each topic gets its own callback
/// URL (e.g., `/webhooks/shopify/orders/create`).
pub async fn reconcile(client: &ShopifyClient, webhook_base_url: &str) -> Result<(), ShopifyError> {
    let existing = list_subscriptions(client).await?;
    info!(
        count = existing.len(),
        "found existing Shopify webhook subscriptions"
    );

    for topic in DESIRED_TOPICS {
        let callback_url = format!(
            "{webhook_base_url}/webhooks/shopify/{}",
            topic_to_path(topic),
        );
        let matching = existing.iter().find(|s| s.topic == *topic);

        match matching {
            Some(sub) if sub.callback_url == callback_url => {
                info!(topic, "subscription already configured");
            }
            Some(sub) => {
                info!(
                    topic,
                    old_url = %sub.callback_url,
                    "updating subscription callback URL"
                );
                update_subscription(client, &sub.id, &callback_url).await?;
            }
            None => {
                info!(topic, "creating webhook subscription");
                create_subscription(client, topic, &callback_url).await?;
            }
        }
    }

    Ok(())
}

/// Convert a GraphQL topic enum value to a URL path segment.
///
/// `ORDERS_CREATE` → `orders/create`
/// `FULFILLMENTS_UPDATE` → `fulfillments/update`
fn topic_to_path(topic: &str) -> String {
    let lower = topic.to_lowercase();
    // Split at the last underscore: resource_action → resource/action.
    // Handles multi-word resources like SUBSCRIPTION_CONTRACTS_CREATE.
    lower
        .rsplit_once('_')
        .map_or(lower.clone(), |(resource, action)| {
            format!("{resource}/{action}")
        })
}

/// An existing webhook subscription.
struct Subscription {
    id: String,
    topic: String,
    callback_url: String,
}

const LIST_QUERY: &str = r"
query {
    webhookSubscriptions(first: 50) {
        edges {
            node {
                id
                topic
                callbackUrl
            }
        }
    }
}
";

const CREATE_MUTATION: &str = r"
mutation webhookCreate($topic: WebhookSubscriptionTopic!, $webhookSubscription: WebhookSubscriptionInput!) {
    webhookSubscriptionCreate(topic: $topic, webhookSubscription: $webhookSubscription) {
        webhookSubscription { id }
        userErrors { field message }
    }
}
";

const UPDATE_MUTATION: &str = r"
mutation webhookUpdate($id: ID!, $webhookSubscription: WebhookSubscriptionInput!) {
    webhookSubscriptionUpdate(id: $id, webhookSubscription: $webhookSubscription) {
        webhookSubscription { id }
        userErrors { field message }
    }
}
";

/// List all webhook subscriptions.
async fn list_subscriptions(client: &ShopifyClient) -> Result<Vec<Subscription>, ShopifyError> {
    let data = client.graphql(LIST_QUERY, serde_json::json!({})).await?;

    let edges = data
        .get("webhookSubscriptions")
        .and_then(|ws| ws.get("edges"))
        .and_then(serde_json::Value::as_array);

    let Some(edges) = edges else {
        return Ok(Vec::new());
    };

    let subs = edges
        .iter()
        .filter_map(|edge| {
            let node = edge.get("node")?;
            Some(Subscription {
                id: node.get("id")?.as_str()?.to_string(),
                topic: node.get("topic")?.as_str()?.to_string(),
                callback_url: node.get("callbackUrl")?.as_str()?.to_string(),
            })
        })
        .collect();

    Ok(subs)
}

/// Create a new webhook subscription.
async fn create_subscription(
    client: &ShopifyClient,
    topic: &str,
    callback_url: &str,
) -> Result<(), ShopifyError> {
    let variables = serde_json::json!({
        "topic": topic,
        "webhookSubscription": {
            "callbackUrl": callback_url,
            "format": "JSON"
        }
    });

    let data = client.graphql(CREATE_MUTATION, variables).await?;
    log_user_errors(&data, "webhookSubscriptionCreate", topic);
    Ok(())
}

/// Update an existing webhook subscription's callback URL.
async fn update_subscription(
    client: &ShopifyClient,
    id: &str,
    callback_url: &str,
) -> Result<(), ShopifyError> {
    let variables = serde_json::json!({
        "id": id,
        "webhookSubscription": {
            "callbackUrl": callback_url,
            "format": "JSON"
        }
    });

    let data = client.graphql(UPDATE_MUTATION, variables).await?;
    log_user_errors(&data, "webhookSubscriptionUpdate", id);
    Ok(())
}

/// Log any `userErrors` from a Shopify mutation response.
fn log_user_errors(data: &serde_json::Value, mutation: &str, context: &str) {
    let errors = data
        .get(mutation)
        .and_then(|r| r.get("userErrors"))
        .and_then(serde_json::Value::as_array);

    let Some(errors) = errors else { return };

    for error in errors {
        if let Some(msg) = error.get("message").and_then(serde_json::Value::as_str) {
            warn!(mutation, context, error = msg, "Shopify mutation user error");
        }
    }
}

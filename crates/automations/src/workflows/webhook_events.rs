//! Webhook event processor.
//!
//! Polls `admin.webhook_event` for pending events and dispatches them to the
//! appropriate workflow handler. Runs on the main database pool with full
//! privileges (unlike the public webhook handlers which use a restricted pool).

use naked_pineapple_services::klaviyo::KlaviyoClient;
use naked_pineapple_services::slack::SlackClient;
use sqlx::PgPool;
use tracing::{debug, error, info, instrument, warn};

use crate::outbound::poller;
use crate::shopify::{self, ShopifyClient, fulfillments};

/// Clients needed for dispatching webhook events to downstream workflows.
pub struct WebhookDispatchClients<'a> {
    pub pool: &'a PgPool,
    pub shopify: Option<&'a ShopifyClient>,
    pub klaviyo: Option<&'a KlaviyoClient>,
    pub slack: Option<&'a SlackClient>,
    /// Internal storefront URL for search index refresh (if configured).
    pub storefront_url: Option<&'a str>,
}

/// A pending webhook event fetched from the database.
struct PendingEvent {
    id: i64,
    source: String,
    event_type: String,
    payload: serde_json::Value,
}

/// Process all pending webhook events.
///
/// Returns `true` if processing completed without critical errors.
#[instrument(skip(clients))]
pub async fn run(clients: &WebhookDispatchClients<'_>) -> bool {
    let events = match fetch_pending(clients.pool).await {
        Ok(events) => events,
        Err(e) => {
            error!(error = %e, "failed to fetch pending webhook events");
            return false;
        }
    };

    if events.is_empty() {
        return true;
    }

    info!(count = events.len(), "processing webhook events");

    for event in &events {
        let result = dispatch(clients, event).await;
        if let Err(e) = mark_processed(clients.pool, event.id, result.err()).await {
            warn!(
                event_id = event.id,
                error = %e,
                "failed to update webhook event status"
            );
        }
    }

    true
}

/// Fetch up to 50 pending events, ordered by arrival time.
async fn fetch_pending(pool: &PgPool) -> Result<Vec<PendingEvent>, sqlx::Error> {
    let rows = sqlx::query_as!(
        PendingEvent,
        r#"
        SELECT id, source, event_type, payload
        FROM admin.webhook_event
        WHERE status = 'pending'
        ORDER BY received_at ASC
        LIMIT 50
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Claim the event and dispatch to the appropriate handler.
async fn dispatch(
    clients: &WebhookDispatchClients<'_>,
    event: &PendingEvent,
) -> Result<(), String> {
    // Claim the event so other scheduler ticks don't re-process it.
    sqlx::query!(
        "UPDATE admin.webhook_event SET status = 'processing' WHERE id = $1",
        event.id,
    )
    .execute(clients.pool)
    .await
    .map_err(|e| e.to_string())?;

    match event.source.as_str() {
        "shopify" => dispatch_shopify(clients, event).await?,
        "github" | "fly" | "sentry" | "betterstack" => {
            dispatch_operational(clients, event).await?;
        }
        other => {
            warn!(
                source = other,
                event_id = event.id,
                "unknown webhook source"
            );
        }
    }

    Ok(())
}

/// Dispatch a Shopify webhook event through the appropriate handler.
///
/// Order/fulfillment events go through the Klaviyo tracking pipeline.
/// Product events trigger storefront search index updates and Slack notifications.
async fn dispatch_shopify(
    clients: &WebhookDispatchClients<'_>,
    event: &PendingEvent,
) -> Result<(), String> {
    match event.event_type.as_str() {
        "orders/create" | "fulfillments/create" | "fulfillments/update" => {
            dispatch_order_event(clients, event).await?;
        }
        "products/create" | "products/update" => {
            handle_product_change(clients, event).await?;
        }
        "products/delete" => {
            handle_product_delete(clients, event).await?;
        }
        other => {
            debug!(event_type = other, "unhandled Shopify webhook event type");
        }
    }

    info!(event_type = %event.event_type, "dispatched Shopify webhook event");
    Ok(())
}

/// Dispatch an order or fulfillment event through the Klaviyo tracking pipeline.
async fn dispatch_order_event(
    clients: &WebhookDispatchClients<'_>,
    event: &PendingEvent,
) -> Result<(), String> {
    let (Some(shopify), Some(klaviyo)) = (clients.shopify, clients.klaviyo) else {
        debug!("Shopify/Klaviyo not configured, skipping order event dispatch");
        return Ok(());
    };

    let order_gid = extract_order_gid(event)?;

    match event.event_type.as_str() {
        "orders/create" => {
            let order = fetch_order(shopify, &order_gid).await?;
            poller::maybe_track_order_confirmed(clients.pool, klaviyo, &order)
                .await
                .map_err(|e| e.to_string())?;
        }
        "fulfillments/create" => {
            let order = fetch_order(shopify, &order_gid).await?;
            poller::maybe_track_order_shipped(clients.pool, klaviyo, &order)
                .await
                .map_err(|e| e.to_string())?;
        }
        "fulfillments/update" if is_delivered(&event.payload) => {
            let order = fetch_order(shopify, &order_gid).await?;
            poller::maybe_track_order_delivered(clients.pool, klaviyo, &order)
                .await
                .map_err(|e| e.to_string())?;
        }
        _ => {}
    }

    Ok(())
}

/// Extract the Shopify order GID from a webhook payload.
///
/// Order webhooks include `admin_graphql_api_id` directly. Fulfillment
/// webhooks include `order_id` (numeric) which is converted to a GID.
fn extract_order_gid(event: &PendingEvent) -> Result<String, String> {
    if event.event_type.starts_with("orders/") {
        event
            .payload
            .get("admin_graphql_api_id")
            .and_then(serde_json::Value::as_str)
            .map(String::from)
            .ok_or_else(|| "missing admin_graphql_api_id in order payload".to_string())
    } else {
        event
            .payload
            .get("order_id")
            .and_then(serde_json::Value::as_i64)
            .map(|id| format!("gid://shopify/Order/{id}"))
            .ok_or_else(|| "missing order_id in fulfillment payload".to_string())
    }
}

/// Check whether a fulfillment webhook payload indicates delivery.
fn is_delivered(payload: &serde_json::Value) -> bool {
    payload
        .get("shipment_status")
        .and_then(serde_json::Value::as_str)
        == Some("delivered")
}

/// Fetch a full order from Shopify by GID, returning an error string on failure.
async fn fetch_order(
    shopify: &ShopifyClient,
    gid: &str,
) -> Result<shopify::fulfillments::OrderDetail, String> {
    fulfillments::fetch_order_by_id(shopify, gid)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("order {gid} not found in Shopify"))
}

/// Dispatch an operational webhook event to Slack.
///
/// Sends a Block Kit notification with source-specific context extracted
/// from the payload. Events are also stored in `admin.webhook_event` for
/// future LLM agent processing.
async fn dispatch_operational(
    clients: &WebhookDispatchClients<'_>,
    event: &PendingEvent,
) -> Result<(), String> {
    use naked_pineapple_services::slack::{Block, ContextElement, PlainText, Text};

    let Some(slack) = clients.slack else {
        debug!("Slack not configured, skipping operational webhook notification");
        return Ok(());
    };

    let source_label = match event.source.as_str() {
        "github" => "GitHub",
        "sentry" => "Sentry",
        "fly" => "Fly.io",
        "betterstack" => "Better Stack",
        other => other,
    };

    let summary = operational_summary(&event.source, &event.payload);

    let blocks = vec![
        Block::Header {
            text: PlainText::new(format!("{source_label} Webhook Event")),
        },
        Block::Section {
            text: Text::mrkdwn(format!("*Event*: `{}`\n{summary}", event.event_type)),
            accessory: None,
        },
        Block::Context {
            elements: vec![ContextElement::Mrkdwn {
                text: format!("Webhook event #{}", event.id),
            }],
        },
    ];

    let channel = slack.default_channel();
    let fallback = format!("{source_label}: {}", event.event_type);

    slack
        .post_message(channel, blocks, Some(&fallback))
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Extract a human-readable summary from an operational webhook payload.
fn operational_summary(source: &str, payload: &serde_json::Value) -> String {
    match source {
        "github" => {
            let action = json_str(payload, &["action"]);
            let repo = json_str(payload, &["repository", "full_name"]);
            format!("*Action*: {action}\n*Repository*: {repo}")
        }
        "sentry" => {
            let title = json_str(payload, &["data", "issue", "title"]);
            let url = json_str(payload, &["data", "issue", "web_url"]);
            format!("*Issue*: {title}\n*URL*: {url}")
        }
        "fly" => {
            let app = json_str(payload, &["app"]);
            format!("*App*: {app}")
        }
        "betterstack" => {
            let cause = json_str(payload, &["data", "attributes", "cause"]);
            format!("*Cause*: {cause}")
        }
        _ => String::new(),
    }
}

/// Walk a JSON path and return the string value, or `"—"` if missing.
fn json_str<'a>(value: &'a serde_json::Value, path: &[&str]) -> &'a str {
    let mut current = value;
    for key in path {
        current = match current.get(*key) {
            Some(v) => v,
            None => return "\u{2014}",
        };
    }
    current.as_str().unwrap_or("\u{2014}")
}

// =============================================================================
// Product webhook handlers
// =============================================================================

/// Handle a product create or update webhook.
///
/// Extracts the product handle, notifies the storefront to refresh its search
/// index, and sends a Slack notification.
async fn handle_product_change(
    clients: &WebhookDispatchClients<'_>,
    event: &PendingEvent,
) -> Result<(), String> {
    let title = json_str(&event.payload, &["title"]);
    let handle = json_str(&event.payload, &["handle"]);
    let action = if event.event_type == "products/create" {
        "created"
    } else {
        "updated"
    };

    // Notify storefront to refresh search index
    if !handle.is_empty() && handle != "\u{2014}" {
        notify_storefront(clients.storefront_url, "refresh", handle).await;
    }

    // Send Slack notification
    send_product_slack_notification(clients.slack, action, title, handle).await;

    Ok(())
}

/// Handle a product delete webhook.
///
/// Notifies the storefront to remove the product from its search index
/// and sends a Slack notification.
async fn handle_product_delete(
    clients: &WebhookDispatchClients<'_>,
    event: &PendingEvent,
) -> Result<(), String> {
    let title = json_str(&event.payload, &["title"]);
    let handle = json_str(&event.payload, &["handle"]);

    // Notify storefront to delete from search index
    if !handle.is_empty() && handle != "\u{2014}" {
        notify_storefront(clients.storefront_url, "delete", handle).await;
    }

    // Send Slack notification
    send_product_slack_notification(clients.slack, "deleted", title, handle).await;

    Ok(())
}

/// Send an HTTP request to the storefront's internal index endpoint.
async fn notify_storefront(storefront_url: Option<&str>, action: &str, handle: &str) {
    let Some(base_url) = storefront_url else {
        debug!("storefront URL not configured, skipping index notification");
        return;
    };

    let endpoint = format!("{base_url}/internal/index/{action}");
    let body = serde_json::json!({ "handle": handle });

    let client = reqwest::Client::new();
    match client.post(&endpoint).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            info!(
                action,
                handle, "notified storefront to {action} product in index"
            );
        }
        Ok(resp) => {
            warn!(
                action,
                handle,
                status = %resp.status(),
                "storefront index notification returned non-success"
            );
        }
        Err(e) => {
            warn!(
                action,
                handle,
                error = %e,
                "failed to notify storefront of product change"
            );
        }
    }
}

/// Send a Slack notification about a product change.
async fn send_product_slack_notification(
    slack: Option<&SlackClient>,
    action: &str,
    title: &str,
    handle: &str,
) {
    use naked_pineapple_services::slack::{Block, ContextElement, PlainText, Text};

    let Some(slack) = slack else {
        return;
    };

    let blocks = vec![
        Block::Header {
            text: PlainText::new(format!("Product {action}")),
        },
        Block::Section {
            text: Text::mrkdwn(format!("*{title}*\nHandle: `{handle}`")),
            accessory: None,
        },
        Block::Context {
            elements: vec![ContextElement::Mrkdwn {
                text: "Shopify product webhook".to_string(),
            }],
        },
    ];

    let channel = slack.default_channel();
    let fallback = format!("Product {action}: {title}");

    if let Err(e) = slack.post_message(channel, blocks, Some(&fallback)).await {
        warn!(
            error = %e,
            "failed to send product change Slack notification"
        );
    }
}

/// Mark an event as processed or failed.
async fn mark_processed(
    pool: &PgPool,
    event_id: i64,
    error: Option<String>,
) -> Result<(), sqlx::Error> {
    let (status, error_message) = error.map_or(("processed", None), |msg| ("failed", Some(msg)));

    sqlx::query!(
        r#"
        UPDATE admin.webhook_event
        SET status = $2,
            error_message = $3,
            processed_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
        WHERE id = $1
        "#,
        event_id,
        status,
        error_message,
    )
    .execute(pool)
    .await?;

    Ok(())
}

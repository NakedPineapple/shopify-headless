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

/// Dispatch a Shopify webhook event through the existing order/fulfillment pipeline.
///
/// Fetches the full order from the Shopify Admin API and feeds it through the
/// same tracking functions the poller uses, deduplicating against `outbound_email_queue`.
async fn dispatch_shopify(
    clients: &WebhookDispatchClients<'_>,
    event: &PendingEvent,
) -> Result<(), String> {
    let (Some(shopify), Some(klaviyo)) = (clients.shopify, clients.klaviyo) else {
        debug!("Shopify/Klaviyo not configured, skipping webhook dispatch");
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
        other => {
            debug!(event_type = other, "unhandled Shopify webhook event type");
        }
    }

    info!(event_type = %event.event_type, "dispatched Shopify webhook event");
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
            text: Text::mrkdwn(format!("*Event*: `{}`\n{summary}", event.event_type,)),
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

//! Pinterest conversion sync workflow.
//!
//! Periodically fetches recent Shopify orders and sends `checkout` conversion
//! events to the Pinterest Conversions API (CAPI). This complements the
//! client-side Pinterest Tag for improved attribution accuracy.
//!
//! Events are deduplicated on Pinterest's side using `event_id` (the Shopify
//! order ID). User data (email) is SHA-256 hashed before sending per Pinterest
//! requirements.

use naked_pineapple_services::pinterest::{
    ConversionCustomData, ConversionEvent, ConversionUserData, PinterestClient,
};
use sha2::{Digest, Sha256};
use tracing::instrument;

use crate::shopify::ShopifyClient;
use crate::shopify::fulfillments::{OrderDetail, fetch_recent_orders};

/// Send conversion events for recent Shopify orders to Pinterest CAPI.
///
/// Returns `true` on success, `false` on failure (for circuit breaker).
#[instrument(skip(shopify, pinterest))]
pub async fn sync_conversions(shopify: &ShopifyClient, pinterest: &PinterestClient) -> bool {
    // Fetch orders from the last 120 minutes (overlap for reliability;
    // Pinterest deduplicates by event_id).
    let orders = match fetch_recent_orders(shopify, 120).await {
        Ok(orders) => orders,
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch recent Shopify orders for Pinterest CAPI");
            return false;
        }
    };

    if orders.is_empty() {
        tracing::debug!("no recent orders for Pinterest conversion sync");
        return true;
    }

    let events: Vec<ConversionEvent> = orders.iter().filter_map(build_conversion_event).collect();

    if events.is_empty() {
        tracing::debug!("no conversion events to send to Pinterest");
        return true;
    }

    let event_count = events.len();
    match pinterest.send_conversion_events(events).await {
        Ok(response) => {
            tracing::info!(
                sent = event_count,
                processed = response.num_events_processed,
                "Pinterest conversion sync complete"
            );
            true
        }
        Err(e) => {
            tracing::error!(error = %e, events = event_count, "failed to send Pinterest conversion events");
            false
        }
    }
}

/// Build a Pinterest conversion event from a Shopify order.
fn build_conversion_event(order: &OrderDetail) -> Option<ConversionEvent> {
    let email = order.email.as_deref()?;

    // Pinterest requires SHA-256 hashed email (lowercase, trimmed)
    let hashed_email = sha256_hash(&email.trim().to_lowercase());

    // Parse the ISO 8601 created_at into a Unix timestamp
    let event_time = chrono::DateTime::parse_from_rfc3339(&order.created_at)
        .map_or_else(|_| chrono::Utc::now().timestamp(), |dt| dt.timestamp());

    Some(ConversionEvent {
        event_name: "checkout".to_string(),
        action_source: "web".to_string(),
        event_time,
        event_id: order.id.clone(),
        event_source_url: None,
        user_data: ConversionUserData {
            em: Some(vec![hashed_email]),
            client_ip_address: None,
            client_user_agent: None,
            external_id: None,
        },
        custom_data: Some(ConversionCustomData {
            currency: Some("USD".to_string()),
            value: Some(order.prices.total.clone()),
            content_ids: None,
            contents: None,
            num_items: Some(order.line_items.len().try_into().unwrap_or(i32::MAX)),
            order_id: Some(order.name.clone()),
        }),
    })
}

/// SHA-256 hash a string and return hex-encoded digest.
fn sha256_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

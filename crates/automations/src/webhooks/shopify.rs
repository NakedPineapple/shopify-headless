//! Shopify webhook handler.
//!
//! Verifies the `X-Shopify-Hmac-Sha256` signature, extracts the deduplication
//! ID from `X-Shopify-Webhook-Id`, and stores the event for async processing.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use secrecy::ExposeSecret;
use tracing::{debug, info, warn};

use super::db;
use super::state::WebhookState;
use super::verify;

/// Handle an inbound Shopify webhook.
///
/// # Route
///
/// `POST /webhooks/shopify/{*topic}`
pub async fn handle(
    State(state): State<WebhookState>,
    Path(topic): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let Some(secret) = state.shopify_secret() else {
        debug!("Shopify webhook secret not configured, ignoring");
        return StatusCode::OK;
    };

    // Verify HMAC signature before any other processing.
    let signature = header_str(&headers, "X-Shopify-Hmac-Sha256");
    if !verify::verify_hmac_sha256_base64(secret.expose_secret().as_bytes(), &body, signature) {
        warn!("Shopify webhook HMAC verification failed");
        return StatusCode::UNAUTHORIZED;
    }

    let external_id = header_str(&headers, "X-Shopify-Webhook-Id");

    let external_id = if external_id.is_empty() {
        None
    } else {
        Some(external_id.to_owned())
    };

    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => {
            // Return 200 so Shopify doesn't retry a malformed payload forever.
            return StatusCode::OK;
        }
    };

    match db::insert_event(
        state.pool(),
        "shopify",
        &topic,
        external_id.as_deref(),
        &payload,
    )
    .await
    {
        Ok(true) => info!(%topic, "Shopify webhook received"),
        Ok(false) => debug!(%topic, "Shopify webhook duplicate, ignored"),
        Err(e) => {
            warn!(error = %e, %topic, "failed to store Shopify webhook event");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }

    StatusCode::OK
}

/// Extract a header value as a string, returning `""` if missing.
fn header_str<'a>(headers: &'a HeaderMap, key: &str) -> &'a str {
    headers.get(key).and_then(|v| v.to_str().ok()).unwrap_or("")
}

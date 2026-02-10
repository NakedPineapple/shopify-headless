//! Sentry webhook handler.
//!
//! Verifies the `Sentry-Hook-Signature` HMAC-SHA256 signature and stores
//! the event for async processing.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use secrecy::ExposeSecret;
use tracing::{debug, info, warn};

use super::db;
use super::state::WebhookState;
use super::verify;

/// Handle an inbound Sentry webhook.
///
/// # Route
///
/// `POST /webhooks/sentry`
pub async fn handle(
    State(state): State<WebhookState>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let Some(secret) = state.sentry_secret() else {
        debug!("Sentry webhook secret not configured, ignoring");
        return StatusCode::OK;
    };

    let signature = header_str(&headers, "Sentry-Hook-Signature");
    if !verify::verify_hmac_sha256_hex(secret.expose_secret().as_bytes(), &body, signature) {
        warn!("Sentry webhook signature verification failed");
        return StatusCode::UNAUTHORIZED;
    }

    let resource = header_str(&headers, "Sentry-Hook-Resource");

    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return StatusCode::OK,
    };

    // Use the event ID from the payload as deduplication key if available.
    let external_id = payload
        .get("data")
        .and_then(|d| d.get("event"))
        .and_then(|e| e.get("event_id"))
        .and_then(serde_json::Value::as_str);

    match db::insert_event(state.pool(), "sentry", resource, external_id, &payload).await {
        Ok(true) => info!(resource, "Sentry webhook received"),
        Ok(false) => debug!(resource, "Sentry webhook duplicate, ignored"),
        Err(e) => {
            warn!(error = %e, resource, "failed to store Sentry webhook event");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }

    StatusCode::OK
}

fn header_str<'a>(headers: &'a HeaderMap, key: &str) -> &'a str {
    headers.get(key).and_then(|v| v.to_str().ok()).unwrap_or("")
}

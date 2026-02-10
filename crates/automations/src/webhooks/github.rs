//! GitHub Actions webhook handler.
//!
//! Verifies the `X-Hub-Signature-256` HMAC-SHA256 signature (hex-encoded),
//! extracts `X-GitHub-Delivery` for deduplication, and stores the event.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use secrecy::ExposeSecret;
use tracing::{debug, info, warn};

use super::db;
use super::state::WebhookState;
use super::verify;

/// Handle an inbound GitHub webhook.
///
/// # Route
///
/// `POST /webhooks/github`
pub async fn handle(
    State(state): State<WebhookState>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let Some(secret) = state.github_secret() else {
        debug!("GitHub webhook secret not configured, ignoring");
        return StatusCode::OK;
    };

    let signature = header_str(&headers, "X-Hub-Signature-256");
    if !verify::verify_hmac_sha256_hex(secret.expose_secret().as_bytes(), &body, signature) {
        warn!("GitHub webhook signature verification failed");
        return StatusCode::UNAUTHORIZED;
    }

    let delivery_id = header_str(&headers, "X-GitHub-Delivery");
    let event_type = header_str(&headers, "X-GitHub-Event");

    let external_id = if delivery_id.is_empty() {
        None
    } else {
        Some(delivery_id)
    };

    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return StatusCode::OK,
    };

    match db::insert_event(state.pool(), "github", event_type, external_id, &payload).await {
        Ok(true) => info!(event_type, "GitHub webhook received"),
        Ok(false) => debug!(event_type, "GitHub webhook duplicate, ignored"),
        Err(e) => {
            warn!(error = %e, event_type, "failed to store GitHub webhook event");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }

    StatusCode::OK
}

fn header_str<'a>(headers: &'a HeaderMap, key: &str) -> &'a str {
    headers.get(key).and_then(|v| v.to_str().ok()).unwrap_or("")
}

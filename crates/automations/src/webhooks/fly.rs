//! Fly.io webhook handler.
//!
//! Verifies the bearer token from the `Authorization` header and stores
//! the event for async processing.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use secrecy::ExposeSecret;
use tracing::{debug, info, warn};

use super::db;
use super::state::WebhookState;
use super::verify;

/// Handle an inbound Fly.io webhook.
///
/// # Route
///
/// `POST /webhooks/fly`
pub async fn handle(
    State(state): State<WebhookState>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let Some(token) = state.fly_token() else {
        debug!("Fly webhook token not configured, ignoring");
        return StatusCode::OK;
    };

    let authorization = header_str(&headers, "Authorization");
    if !verify::verify_bearer_token(token.expose_secret(), authorization) {
        warn!("Fly webhook token verification failed");
        return StatusCode::UNAUTHORIZED;
    }

    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return StatusCode::OK,
    };

    // Extract event type and ID from the payload.
    let event_type = payload
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let external_id = payload
        .get("id")
        .and_then(serde_json::Value::as_str);

    match db::insert_event(state.pool(), "fly", event_type, external_id, &payload).await {
        Ok(true) => info!(event_type, "Fly webhook received"),
        Ok(false) => debug!(event_type, "Fly webhook duplicate, ignored"),
        Err(e) => {
            warn!(error = %e, event_type, "failed to store Fly webhook event");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }

    StatusCode::OK
}

fn header_str<'a>(headers: &'a HeaderMap, key: &str) -> &'a str {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

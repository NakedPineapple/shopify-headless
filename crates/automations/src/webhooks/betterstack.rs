//! Better Stack webhook handler.
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

/// Handle an inbound Better Stack webhook.
///
/// # Route
///
/// `POST /webhooks/betterstack`
pub async fn handle(
    State(state): State<WebhookState>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let Some(secret) = state.betterstack_secret() else {
        debug!("Better Stack webhook secret not configured, ignoring");
        return StatusCode::OK;
    };

    let authorization = header_str(&headers, "Authorization");
    if !verify::verify_bearer_token(secret.expose_secret(), authorization) {
        warn!("Better Stack webhook token verification failed");
        return StatusCode::UNAUTHORIZED;
    }

    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return StatusCode::OK,
    };

    let event_type = payload
        .get("data")
        .and_then(|d| d.get("attributes"))
        .and_then(|a| a.get("cause"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("incident");

    let external_id = payload
        .get("data")
        .and_then(|d| d.get("id"))
        .and_then(serde_json::Value::as_str);

    match db::insert_event(state.pool(), "betterstack", event_type, external_id, &payload).await {
        Ok(true) => info!(event_type, "Better Stack webhook received"),
        Ok(false) => debug!(event_type, "Better Stack webhook duplicate, ignored"),
        Err(e) => {
            warn!(error = %e, event_type, "failed to store Better Stack webhook event");
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

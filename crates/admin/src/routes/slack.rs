//! Slack webhook handler for interaction responses.
//!
//! Handles button clicks from Slack confirmation messages.

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use crate::error::AppError;
use crate::services::ActionQueueService;
use crate::slack::InteractionPayload;
use crate::state::AppState;

/// Create Slack webhook routes.
pub fn router() -> Router<AppState> {
    Router::new().route("/api/slack/interactions", post(handle_interaction))
}

/// Handle Slack interaction webhook.
///
/// Receives button clicks from confirmation messages and processes
/// approval or rejection of pending actions.
#[instrument(skip(state, headers, body))]
async fn handle_interaction(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, AppError> {
    // Extract headers for signature verification
    let timestamp = headers
        .get("X-Slack-Request-Timestamp")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("Missing timestamp header".into()))?;

    let signature = headers
        .get("X-Slack-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("Missing signature header".into()))?;

    // Verify signature
    let slack = state
        .slack()
        .ok_or_else(|| AppError::Internal("Slack not configured".into()))?;

    slack
        .verify_signature(timestamp, &body, signature)
        .map_err(|e| AppError::Unauthorized(e.to_string()))?;

    debug!("Slack signature verified");

    // Parse the payload (URL-encoded)
    let payload_str = body
        .strip_prefix("payload=")
        .ok_or_else(|| AppError::BadRequest("Invalid payload format".into()))?;

    let payload_decoded = urlencoding::decode(payload_str)
        .map_err(|e| AppError::BadRequest(format!("Failed to decode payload: {e}")))?;

    let payload: InteractionPayload = serde_json::from_str(&payload_decoded)
        .map_err(|e| AppError::BadRequest(format!("Failed to parse payload: {e}")))?;

    // Handle the interaction
    let action = payload
        .actions
        .first()
        .ok_or_else(|| AppError::BadRequest("No actions in payload".into()))?;

    let action_id = action
        .value
        .as_ref()
        .and_then(|v| Uuid::parse_str(v).ok())
        .ok_or_else(|| AppError::BadRequest("Invalid action ID".into()))?;

    let user_name = payload
        .user
        .name
        .as_deref()
        .or(payload.user.username.as_deref())
        .unwrap_or(&payload.user.id);

    // Determine if this is an approval or rejection
    let is_approval = action.action_id.starts_with("approve_");
    let is_rejection = action.action_id.starts_with("reject_");

    if !is_approval && !is_rejection {
        warn!(action_id = %action.action_id, "Unknown action type");
        return Err(AppError::BadRequest("Unknown action type".into()));
    }

    // Create action queue service
    let action_queue = ActionQueueService::new(state.db(), state.slack().cloned());

    if is_approval {
        info!(action_id = %action_id, user = %user_name, "Processing approval");
        match action_queue
            .approve(action_id, user_name, state.shopify())
            .await
        {
            Ok(_result) => {
                debug!(action_id = %action_id, "Action approved and executed");
            }
            Err(e) => {
                error!(action_id = %action_id, error = %e, "Approval failed");
                // Don't return error - Slack has already been updated by the service
            }
        }
    } else {
        info!(action_id = %action_id, user = %user_name, "Processing rejection");
        if let Err(e) = action_queue.reject(action_id, user_name).await {
            error!(action_id = %action_id, error = %e, "Rejection failed");
        }
    }

    // Return 200 OK to Slack (always, even on errors - we handle updates separately)
    Ok(StatusCode::OK)
}

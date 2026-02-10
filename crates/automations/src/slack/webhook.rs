//! Slack interactive webhook handler.
//!
//! Handles approve/reject button clicks from email review messages.
//! Verifies Slack signatures, processes actions, and responds.

use axum::Form;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use naked_pineapple_services::slack::{InteractionPayload, SlackClient};
use tracing::{error, info, instrument, warn};

use crate::db::inbound_email;
use crate::slack::messages;
use crate::state::AppState;

/// Form data from Slack interactive webhook.
#[derive(serde::Deserialize)]
pub struct SlackInteractionForm {
    payload: String,
}

/// Handle Slack interactive webhook (button clicks).
///
/// Slack sends a POST with `application/x-www-form-urlencoded` body containing
/// a `payload` field with JSON.
#[instrument(skip(state, headers, form))]
pub async fn handle_interaction(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<SlackInteractionForm>,
) -> StatusCode {
    let Some(slack) = state.slack() else {
        warn!("received Slack webhook but Slack is not configured");
        return StatusCode::OK;
    };

    // Verify signature
    if let Err(e) = verify_slack_signature(&headers, &form.payload, slack) {
        warn!(error = %e, "Slack signature verification failed");
        return StatusCode::UNAUTHORIZED;
    }

    // Parse payload
    let payload: InteractionPayload = match serde_json::from_str(&form.payload) {
        Ok(p) => p,
        Err(e) => {
            error!(error = %e, "failed to parse Slack interaction payload");
            return StatusCode::BAD_REQUEST;
        }
    };

    // Process actions
    for action in &payload.actions {
        if let Err(e) = process_action(&state, slack, &payload, action).await {
            error!(
                action_id = %action.action_id,
                error = %e,
                "failed to process Slack action"
            );
        }
    }

    StatusCode::OK
}

/// Verify the Slack request signature.
fn verify_slack_signature(
    headers: &HeaderMap,
    body: &str,
    slack: &SlackClient,
) -> Result<(), String> {
    let timestamp = headers
        .get("X-Slack-Request-Timestamp")
        .and_then(|v| v.to_str().ok())
        .ok_or("missing X-Slack-Request-Timestamp header")?;

    let signature = headers
        .get("X-Slack-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or("missing X-Slack-Signature header")?;

    // Reconstruct the raw form body for signature verification
    let raw_body = format!("payload={}", urlencoding::encode(body));

    slack
        .verify_signature(timestamp, &raw_body, signature)
        .map_err(|e| e.to_string())
}

/// Process a single Slack action.
async fn process_action(
    state: &AppState,
    slack: &SlackClient,
    payload: &InteractionPayload,
    action: &naked_pineapple_services::slack::InteractionAction,
) -> Result<(), ActionError> {
    let action_id = &action.action_id;

    if let Some(email_id_str) = action_id.strip_prefix("email_approve_") {
        let email_id: i32 = email_id_str.parse().map_err(|_| ActionError::InvalidId)?;
        return handle_approve(state, slack, payload, email_id).await;
    }

    if let Some(email_id_str) = action_id.strip_prefix("email_reject_") {
        let email_id: i32 = email_id_str.parse().map_err(|_| ActionError::InvalidId)?;
        return handle_reject(state, slack, payload, email_id).await;
    }

    warn!(action_id = %action_id, "unknown Slack action");
    Ok(())
}

/// Handle an approve action: send the draft reply via M365 and update Slack.
async fn handle_approve(
    state: &AppState,
    slack: &SlackClient,
    payload: &InteractionPayload,
    email_id: i32,
) -> Result<(), ActionError> {
    info!(email_id, "email response approved");

    let pool = state.pool();

    // Fetch reply info from DB
    let reply_info = inbound_email::get_reply_info(pool, email_id).await?;

    let draft = reply_info
        .response_draft
        .as_deref()
        .ok_or(ActionError::NoDraft)?;

    // Send reply via M365
    state
        .m365()
        .reply(&reply_info.mailbox, &reply_info.m365_message_id, draft)
        .await?;

    // Update DB
    inbound_email::mark_response_sent(pool, email_id).await?;

    // Update Slack message
    let approved_by = payload
        .user
        .name
        .as_deref()
        .or(payload.user.username.as_deref())
        .unwrap_or("Unknown");

    if let Some(response_url) = &payload.response_url {
        let blocks = messages::build_email_approved_message(
            &reply_info.from_address,
            "(approved)",
            approved_by,
        );
        if let Err(e) = slack.respond_to_url(response_url, blocks, true).await {
            error!(error = %e, "failed to update Slack message after approval");
        }
    }

    Ok(())
}

/// Handle a reject action: mark as rejected and update Slack.
async fn handle_reject(
    state: &AppState,
    slack: &SlackClient,
    payload: &InteractionPayload,
    email_id: i32,
) -> Result<(), ActionError> {
    info!(email_id, "email response rejected");

    let pool = state.pool();

    let rejected_by = payload
        .user
        .name
        .as_deref()
        .or(payload.user.username.as_deref())
        .unwrap_or("Unknown");

    // Update DB
    inbound_email::mark_review_rejected(pool, email_id, rejected_by).await?;

    // Fetch from address for Slack message
    let reply_info = inbound_email::get_reply_info(pool, email_id).await?;

    // Update Slack message
    if let Some(response_url) = &payload.response_url {
        let blocks = messages::build_email_rejected_message(
            &reply_info.from_address,
            "(rejected)",
            rejected_by,
        );
        if let Err(e) = slack.respond_to_url(response_url, blocks, true).await {
            error!(error = %e, "failed to update Slack message after rejection");
        }
    }

    Ok(())
}

/// Errors during action processing.
#[derive(Debug, thiserror::Error)]
enum ActionError {
    #[error("invalid email ID")]
    InvalidId,
    #[error("no draft response found")]
    NoDraft,
    #[error("database error: {0}")]
    Database(#[from] crate::db::RepositoryError),
    #[error("M365 error: {0}")]
    M365(#[from] naked_pineapple_services::microsoft_graph::M365Error),
}

//! Slack interactive webhook handler.
//!
//! Handles approve/reject button clicks from email review messages.
//! Verifies Slack signatures, processes actions, and responds.

use axum::Form;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use naked_pineapple_services::klaviyo::KlaviyoClient;
use naked_pineapple_services::slack::{InteractionPayload, SlackClient};
use naked_pineapple_support::db::conversation::ConversationRepository;
use naked_pineapple_support::models::CreateConversationParams;
use sqlx::PgPool;
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

    if let Some(id_str) = action_id.strip_prefix("email_approve_") {
        let email_id: i32 = id_str.parse().map_err(|_| ActionError::InvalidId)?;
        return handle_approve(state, slack, payload, email_id).await;
    }

    if let Some(id_str) = action_id.strip_prefix("email_reject_") {
        let email_id: i32 = id_str.parse().map_err(|_| ActionError::InvalidId)?;
        return handle_reject(state, slack, payload, email_id).await;
    }

    if let Some(id_str) = action_id.strip_prefix("helpdesk_approve_") {
        let email_id: i32 = id_str.parse().map_err(|_| ActionError::InvalidId)?;
        return handle_helpdesk_approve(state, slack, payload, email_id).await;
    }

    if let Some(id_str) = action_id.strip_prefix("helpdesk_reject_") {
        let email_id: i32 = id_str.parse().map_err(|_| ActionError::InvalidId)?;
        return handle_helpdesk_reject(state, slack, payload, email_id).await;
    }

    warn!(action_id = %action_id, "unknown Slack action");
    Ok(())
}

/// Handle an approve action: send the draft reply via M365, create support
/// conversation, and update Slack.
async fn handle_approve(
    state: &AppState,
    slack: &SlackClient,
    payload: &InteractionPayload,
    email_id: i32,
) -> Result<(), ActionError> {
    info!(email_id, "email response approved");

    let pool = state.pool();

    // Fetch review info from DB
    let review_info = inbound_email::get_review_info(pool, email_id).await?;

    let draft = review_info
        .response_draft
        .as_deref()
        .ok_or(ActionError::NoDraft)?;

    // Send reply via M365
    state
        .m365()
        .reply(&review_info.mailbox, &review_info.m365_message_id, draft)
        .await?;

    // Update DB
    inbound_email::mark_response_sent(pool, email_id).await?;

    // Create support conversation now that a human has approved
    if let Some(support_pool) = state.support_pool() {
        create_support_conversation(
            support_pool,
            email_id,
            &review_info.from_address,
            review_info.from_name.as_deref(),
        )
        .await;
    }

    // Update Slack message
    let approved_by = reviewer_name(payload);

    if let Some(response_url) = &payload.response_url {
        let blocks = messages::build_email_approved_message(
            &review_info.from_address,
            &review_info.subject,
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
    let rejected_by = reviewer_name(payload);

    // Update DB
    inbound_email::mark_review_rejected(pool, email_id, rejected_by).await?;

    // Fetch info for Slack message
    let review_info = inbound_email::get_review_info(pool, email_id).await?;

    // Update Slack message
    if let Some(response_url) = &payload.response_url {
        let blocks = messages::build_email_rejected_message(
            &review_info.from_address,
            &review_info.subject,
            rejected_by,
        );
        if let Err(e) = slack.respond_to_url(response_url, blocks, true).await {
            error!(error = %e, "failed to update Slack message after rejection");
        }
    }

    Ok(())
}

/// Handle helpdesk approve: create Klaviyo ticket and update Slack.
async fn handle_helpdesk_approve(
    state: &AppState,
    slack: &SlackClient,
    payload: &InteractionPayload,
    email_id: i32,
) -> Result<(), ActionError> {
    info!(email_id, "helpdesk routing approved");

    let pool = state.pool();
    let review_info = inbound_email::get_review_info(pool, email_id).await?;
    let approved_by = reviewer_name(payload);

    // Create Klaviyo helpdesk ticket
    if let Some(klaviyo) = state.klaviyo() {
        create_klaviyo_ticket(klaviyo, email_id, &review_info).await;
    }

    // Update DB
    inbound_email::mark_review_approved(pool, email_id, approved_by).await?;

    // Update Slack message
    if let Some(response_url) = &payload.response_url {
        let blocks = messages::build_helpdesk_approved_message(
            &review_info.from_address,
            &review_info.subject,
            approved_by,
        );
        if let Err(e) = slack.respond_to_url(response_url, blocks, true).await {
            error!(error = %e, "failed to update Slack message after helpdesk approval");
        }
    }

    Ok(())
}

/// Handle helpdesk reject: dismiss without creating a ticket.
async fn handle_helpdesk_reject(
    state: &AppState,
    slack: &SlackClient,
    payload: &InteractionPayload,
    email_id: i32,
) -> Result<(), ActionError> {
    info!(email_id, "helpdesk routing dismissed");

    let pool = state.pool();
    let dismissed_by = reviewer_name(payload);

    // Update DB
    inbound_email::mark_review_rejected(pool, email_id, dismissed_by).await?;

    // Fetch info for Slack message
    let review_info = inbound_email::get_review_info(pool, email_id).await?;

    // Update Slack message
    if let Some(response_url) = &payload.response_url {
        let blocks = messages::build_helpdesk_dismissed_message(
            &review_info.from_address,
            &review_info.subject,
            dismissed_by,
        );
        if let Err(e) = slack.respond_to_url(response_url, blocks, true).await {
            error!(error = %e, "failed to update Slack message after helpdesk dismissal");
        }
    }

    Ok(())
}

/// Extract the reviewer's display name from the Slack payload.
fn reviewer_name(payload: &InteractionPayload) -> &str {
    payload
        .user
        .name
        .as_deref()
        .or(payload.user.username.as_deref())
        .unwrap_or("Unknown")
}

/// Create a Klaviyo helpdesk event from stored email review info.
async fn create_klaviyo_ticket(
    klaviyo: &KlaviyoClient,
    email_id: i32,
    info: &inbound_email::EmailReviewInfo,
) {
    let classification = info.classification.as_deref().unwrap_or("unknown");
    let reasoning = info.reasoning.as_deref().unwrap_or("");

    let params = naked_pineapple_services::klaviyo::HelpdeskEventParams {
        email: &info.from_address,
        customer_name: info.from_name.as_deref(),
        subject: &info.subject,
        classification,
        reasoning,
        email_id,
    };

    if let Err(e) = klaviyo.track_helpdesk_event(&params).await {
        error!(email_id, error = %e, "failed to track helpdesk event in Klaviyo");
    }
}

/// Create a support conversation in the storefront DB.
async fn create_support_conversation(
    support_pool: &PgPool,
    email_id: i32,
    from_address: &str,
    from_name: Option<&str>,
) {
    let repo = ConversationRepository::new(support_pool);
    let params = CreateConversationParams {
        session_token: format!("email-{email_id}"),
        shopify_customer_id: None,
        customer_email: Some(from_address.to_string()),
        customer_name: from_name.map(String::from),
        is_authenticated: false,
        source: Some("email".to_string()),
    };

    match repo.create(&params).await {
        Ok(conv) => {
            info!(
                email_id,
                conversation_id = conv.id.as_i32(),
                "support conversation created from email"
            );
        }
        Err(e) => {
            error!(email_id, error = %e, "failed to create support conversation from email");
        }
    }
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

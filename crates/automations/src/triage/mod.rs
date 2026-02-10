//! Email triage pipeline.
//!
//! Orchestrates the two-step AI triage process:
//! 1. **Classification** — Claude classifies the email and extracts entities
//! 2. **Routing** — Based on classification, the email is archived, queued for
//!    Slack review, or routed to Klaviyo Helpdesk

pub mod classifier;
pub mod responder;
pub mod router;
pub mod tools;
pub mod types;

use std::collections::HashMap;

use naked_pineapple_services::claude::ClaudeClient;
use naked_pineapple_services::klaviyo::KlaviyoClient;
use naked_pineapple_services::slack::SlackClient;
use sqlx::PgPool;
use tracing::{error, info, instrument, warn};

use crate::db::inbound_email;
use crate::shopify::ShopifyClient;
use crate::triage::classifier::EmailContext;
use crate::triage::types::EmailStatus;
use naked_pineapple_services::microsoft_graph::{GraphMessage, M365Client};

/// Service clients used by the triage pipeline.
pub struct TriageClients<'a> {
    pub pool: &'a PgPool,
    pub m365: &'a M365Client,
    pub claude: &'a ClaudeClient,
    pub slack: Option<&'a SlackClient>,
    pub klaviyo: Option<&'a KlaviyoClient>,
    pub shopify: Option<&'a ShopifyClient>,
    pub support_pool: Option<&'a PgPool>,
}

/// Process a batch of messages from a single mailbox.
///
/// For each message:
/// 1. Check if already processed (deduplicate by `m365_message_id`)
/// 2. Store in database (with folder name and read status)
/// 3. Classify with Claude AI
/// 4. Route based on classification
/// 5. Mark as read in M365
#[instrument(skip(clients, messages, folder_map), fields(mailbox = %mailbox, count = messages.len()))]
pub async fn process_messages(
    clients: &TriageClients<'_>,
    mailbox: &str,
    messages: Vec<GraphMessage>,
    folder_map: &HashMap<String, String>,
) {
    for message in &messages {
        let folder_name = message
            .parent_folder_id
            .as_deref()
            .and_then(|id| folder_map.get(id))
            .map(String::as_str);

        if let Err(e) = process_single_message(clients, mailbox, message, folder_name).await {
            error!(
                m365_id = message.id,
                error = %e,
                "failed to process message"
            );
        }
    }
}

/// Process a single inbound email message.
async fn process_single_message(
    clients: &TriageClients<'_>,
    mailbox: &str,
    message: &GraphMessage,
    folder_name: Option<&str>,
) -> Result<(), router::TriageError> {
    let m365_id = &message.id;

    // Deduplicate: skip if already processed
    if inbound_email::exists_by_m365_id(clients.pool, m365_id).await? {
        return Ok(());
    }

    // Extract fields from the GraphMessage
    let conversation_id = message.conversation_id.as_deref().unwrap_or("");
    let from_address = extract_from_address(message);
    let from_name = extract_from_name(message);
    let subject = message.subject.as_deref().unwrap_or("(no subject)");
    let body_preview = message.body_preview.as_deref().unwrap_or("");
    let body_text = extract_body_text(message);
    let to_addresses = extract_to_addresses(message);
    let received_at = message.received_date_time.unwrap_or_else(chrono::Utc::now);
    let is_read = message.is_read.unwrap_or(false);

    // Store in database
    let email_id = inbound_email::insert(
        clients.pool,
        &inbound_email::InsertParams {
            m365_message_id: m365_id,
            conversation_id,
            mailbox,
            from_address: &from_address,
            from_name: from_name.as_deref(),
            to_addresses: &to_addresses,
            subject,
            body_preview,
            body_text: &body_text,
            received_at,
            folder: folder_name,
            is_read,
        },
    )
    .await?;

    info!(email_id, from = %from_address, %subject, "stored inbound email");

    // Load thread context
    let thread_messages =
        inbound_email::get_thread_context(clients.pool, conversation_id, email_id)
            .await
            .unwrap_or_default();

    let thread_context: Vec<classifier::ThreadMessage> = thread_messages
        .into_iter()
        .map(|m| classifier::ThreadMessage {
            from: m.from_address,
            body_preview: m.body_preview,
        })
        .collect();

    // Step 1: Classify
    let context = EmailContext {
        from_address: from_address.clone(),
        from_name: from_name.clone(),
        subject: subject.to_string(),
        body: body_text.clone(),
        thread_context,
    };

    let classification = match classifier::classify_email(clients.claude, &context).await {
        Ok(c) => c,
        Err(e) => {
            warn!(email_id, error = %e, "classification failed");
            inbound_email::update_status(clients.pool, email_id, EmailStatus::Failed, None).await?;
            inbound_email::set_error(clients.pool, email_id, &e.to_string()).await?;
            return Ok(());
        }
    };

    // Save classification to DB
    inbound_email::save_classification(clients.pool, email_id, &classification).await?;

    // Step 2: Route
    let route_params = router::RouteParams {
        email_id,
        mailbox,
        m365_message_id: m365_id,
        from_address: &from_address,
        from_name: from_name.as_deref(),
        subject,
        body: &body_text,
        classification: &classification,
        conversation_id,
        support_pool: clients.support_pool,
    };

    router::route_email(
        clients.pool,
        clients.m365,
        clients.claude,
        clients.slack,
        clients.klaviyo,
        clients.shopify,
        &route_params,
    )
    .await?;

    // Mark as read in M365
    if let Err(e) = clients.m365.mark_read(mailbox, m365_id).await {
        warn!(email_id, error = %e, "failed to mark message as read in M365");
    }

    Ok(())
}

/// Extract sender email address from a `GraphMessage`.
fn extract_from_address(message: &GraphMessage) -> String {
    message
        .from
        .as_ref()
        .and_then(|f| f.email_address.address.as_deref())
        .unwrap_or("unknown@unknown.com")
        .to_string()
}

/// Extract sender display name from a `GraphMessage`.
fn extract_from_name(message: &GraphMessage) -> Option<String> {
    message
        .from
        .as_ref()
        .and_then(|f| f.email_address.name.clone())
}

/// Extract plain text body from a `GraphMessage`.
fn extract_body_text(message: &GraphMessage) -> String {
    message
        .body
        .as_ref()
        .and_then(|b| b.content.clone())
        .unwrap_or_default()
}

/// Extract recipient addresses as a JSON-serializable list.
fn extract_to_addresses(message: &GraphMessage) -> serde_json::Value {
    let addresses: Vec<String> = message
        .to_recipients
        .as_ref()
        .map(|recipients| {
            recipients
                .iter()
                .filter_map(|r| r.email_address.address.clone())
                .collect()
        })
        .unwrap_or_default();

    serde_json::to_value(addresses).unwrap_or(serde_json::Value::Array(vec![]))
}

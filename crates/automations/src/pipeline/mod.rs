//! Full triage pipeline: classify → route → M365 integration.
//!
//! This module combines the library's classification and graph update logic
//! with binary-only routing (Slack, Shopify, M365). It is only available
//! in the automations binary, not the library.

pub mod responder;
pub mod router;

use std::collections::HashMap;

use naked_pineapple_services::claude::ClaudeClient;
use naked_pineapple_services::openai::EmbeddingClient;
use naked_pineapple_services::slack::SlackClient;
use sqlx::PgPool;
use tracing::{error, info, instrument, warn};

use crate::db::{contact_graph, inbound_email};
use crate::shopify::ShopifyClient;
use crate::triage::classifier::{self, EmailContext};
use crate::triage::graph_updater;
use crate::triage::types::EmailStatus;
use naked_pineapple_services::microsoft_graph::{GraphMessage, M365Client};

/// Service clients used by the triage pipeline.
pub struct TriageClients<'a> {
    pub pool: &'a PgPool,
    pub m365: &'a M365Client,
    pub claude: &'a ClaudeClient,
    pub embedding: Option<&'a EmbeddingClient>,
    pub slack: Option<&'a SlackClient>,
    pub shopify: Option<&'a ShopifyClient>,
}

/// Process a batch of messages from a single mailbox.
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

    // Deduplicate: skip if already processed; re-analyze if reset to pending
    let email_id = match inbound_email::check_existing(clients.pool, m365_id).await? {
        inbound_email::Existing::Processed => return Ok(()),
        inbound_email::Existing::Pending(id) => {
            info!(email_id = id, "re-analyzing previously reset email");
            id
        }
        inbound_email::Existing::No => {
            let id = store_new_message(clients.pool, mailbox, message, folder_name).await?;
            let from = extract_from_address(message);
            let subject = message.subject.as_deref().unwrap_or("(no subject)");
            info!(email_id = id, from = %from, %subject, "stored inbound email");
            id
        }
    };

    let email = inbound_email::get_for_analysis(clients.pool, email_id).await?;
    analyze_and_route(clients, email_id, &email, m365_id, mailbox).await
}

/// Store a new inbound email in the database.
async fn store_new_message(
    pool: &PgPool,
    mailbox: &str,
    message: &GraphMessage,
    folder_name: Option<&str>,
) -> Result<i32, router::TriageError> {
    let conversation_id = message.conversation_id.as_deref().unwrap_or("");
    let from_address = extract_from_address(message);
    let from_name = extract_from_name(message);
    let subject = message.subject.as_deref().unwrap_or("(no subject)");
    let body_preview = message.body_preview.as_deref().unwrap_or("");
    let body_text = extract_body_text(message);
    let to_addresses = extract_to_addresses(message);
    let received_at = message.received_date_time.unwrap_or_else(chrono::Utc::now);
    let is_read = message.is_read.unwrap_or(false);

    let id = inbound_email::insert(
        pool,
        &inbound_email::InsertParams {
            m365_message_id: &message.id,
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

    Ok(id)
}

/// Run the full analysis pipeline: enrich → classify → graph update → route.
async fn analyze_and_route(
    clients: &TriageClients<'_>,
    email_id: i32,
    email: &inbound_email::EmailForAnalysis,
    m365_message_id: &str,
    mailbox: &str,
) -> Result<(), router::TriageError> {
    let thread_messages =
        inbound_email::get_thread_context(clients.pool, &email.conversation_id, email_id)
            .await
            .unwrap_or_default();

    let thread_context: Vec<classifier::ThreadMessage> = thread_messages
        .into_iter()
        .map(|m| classifier::ThreadMessage {
            from: m.from_address,
            body_preview: m.body_preview,
        })
        .collect();

    let sender_context = enrich_sender_context(clients.pool, &email.from_address).await;

    let (embedding, rag_context) = retrieve_rag_context(
        clients.embedding,
        clients.pool,
        &email.subject,
        &email.body_text,
        email_id,
    )
    .await;

    let context = EmailContext {
        from_address: email.from_address.clone(),
        from_name: email.from_name.clone(),
        subject: email.subject.clone(),
        body: email.body_text.clone(),
        thread_context,
        sender_context,
        rag_context,
    };

    let classification = match classifier::classify_email(clients.claude, clients.pool, &context)
        .await
    {
        Ok(c) => c,
        Err(e) => {
            warn!(email_id, error = %e, "classification failed");
            inbound_email::update_status(clients.pool, email_id, EmailStatus::Failed, None).await?;
            inbound_email::set_error(clients.pool, email_id, &e.to_string()).await?;
            return Ok(());
        }
    };

    inbound_email::save_classification(clients.pool, email_id, &classification).await?;

    if let Some(ref emb) = embedding
        && let Err(e) = inbound_email::save_embedding(clients.pool, email_id, emb).await
    {
        warn!(email_id, error = %e, "failed to save email embedding");
    }

    match graph_updater::extract_graph_updates(clients.claude, &context, &classification).await {
        Ok(Some(updates)) => {
            match graph_updater::apply_graph_updates(clients.pool, &updates).await {
                Ok(contributions) => {
                    if let Err(e) =
                        contact_graph::record_contributions(clients.pool, email_id, &contributions)
                            .await
                    {
                        warn!(email_id, error = %e, "failed to record graph contributions");
                    }
                }
                Err(e) => warn!(email_id, error = %e, "failed to apply graph updates"),
            }
        }
        Ok(None) => {}
        Err(e) => warn!(email_id, error = %e, "graph update extraction failed"),
    }

    let route_params = router::RouteParams {
        email_id,
        mailbox,
        m365_message_id,
        from_address: &email.from_address,
        from_name: email.from_name.as_deref(),
        subject: &email.subject,
        body: &email.body_text,
        classification: &classification,
        conversation_id: &email.conversation_id,
    };

    router::route_email(
        clients.pool,
        clients.m365,
        clients.claude,
        clients.slack,
        clients.shopify,
        &route_params,
    )
    .await?;

    if let Err(e) = clients.m365.mark_read(mailbox, m365_message_id).await {
        warn!(email_id, error = %e, "failed to mark message as read in M365");
    }

    Ok(())
}

fn extract_from_address(message: &GraphMessage) -> String {
    message
        .from
        .as_ref()
        .and_then(|f| f.email_address.address.as_deref())
        .unwrap_or("unknown@unknown.com")
        .to_string()
}

fn extract_from_name(message: &GraphMessage) -> Option<String> {
    message
        .from
        .as_ref()
        .and_then(|f| f.email_address.name.clone())
}

fn extract_body_text(message: &GraphMessage) -> String {
    message
        .body
        .as_ref()
        .and_then(|b| b.content.clone())
        .unwrap_or_default()
}

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

fn strip_html(html: &str) -> String {
    if !html.contains('<') {
        return html.to_string();
    }
    html2text::from_read(html.as_bytes(), 80).unwrap_or_else(|_| html.to_string())
}

async fn enrich_sender_context(pool: &PgPool, from_address: &str) -> Option<String> {
    let domain = from_address.rsplit_once('@').map_or("", |(_, d)| d);

    match contact_graph::lookup_sender(pool, from_address, domain).await {
        Ok(ctx) => ctx,
        Err(e) => {
            warn!(error = %e, "contact graph sender lookup failed");
            None
        }
    }
}

async fn retrieve_rag_context(
    embedding_client: Option<&EmbeddingClient>,
    pool: &PgPool,
    subject: &str,
    body_html: &str,
    exclude_id: i32,
) -> (Option<Vec<f32>>, Option<String>) {
    let Some(client) = embedding_client else {
        return (None, None);
    };

    let plain_text = strip_html(body_html);
    let mut embed_input = format!("{subject}\n\n{plain_text}");
    embed_input.truncate(8000);

    let embedding = match client.embed(&embed_input).await {
        Ok(emb) => emb,
        Err(e) => {
            warn!(error = %e, "failed to generate email embedding");
            return (None, None);
        }
    };

    let similar = match inbound_email::search_similar(pool, &embedding, 5, Some(exclude_id)).await {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "similar email search failed");
            return (Some(embedding), None);
        }
    };

    let relevant: Vec<_> = similar
        .into_iter()
        .filter(|s| s.similarity >= 0.3)
        .collect();

    if relevant.is_empty() {
        return (Some(embedding), None);
    }

    let formatted = format_rag_context(&relevant);
    (Some(embedding), Some(formatted))
}

fn format_rag_context(similar: &[inbound_email::SimilarEmail]) -> String {
    use std::fmt::Write;
    let mut output = String::new();

    for (i, email) in similar.iter().enumerate() {
        let _ = writeln!(output, "{}. From: {}", i + 1, email.from_address);
        let _ = writeln!(output, "   Subject: {}", email.subject);
        if let Some(ref cls) = email.classification {
            let _ = writeln!(output, "   Classification: {cls}");
        }
        let preview = if email.body_preview.len() > 200 {
            format!("{}...", &email.body_preview[..200])
        } else {
            email.body_preview.clone()
        };
        let _ = writeln!(output, "   Preview: {preview}");
        let _ = writeln!(output, "   Similarity: {:.0}%", email.similarity * 100.0);
    }

    output
}

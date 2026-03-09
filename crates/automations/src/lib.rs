//! Pineapple Skin Co. Automations — shared library for the automations service.
//!
//! Exposes [`reanalyze_email`] so the admin panel can synchronously re-run
//! the AI classification pipeline on an email that was previously analyzed.
//! Routing, Slack, and Shopify integrations remain private to the binary.

#![cfg_attr(not(test), forbid(unsafe_code))]

pub mod db;
pub mod triage;

use naked_pineapple_services::claude::ClaudeClient;
use naked_pineapple_services::openai::EmbeddingClient;
use sqlx::PgPool;
use thiserror::Error;
use tracing::warn;

use db::{contact_graph, inbound_email};
use triage::classifier::{self, EmailContext};
use triage::graph_updater;
use triage::types::EmailStatus;

/// Error returned by [`reanalyze_email`].
#[derive(Debug, Error)]
pub enum ReanalyzeError {
    /// Database operation failed.
    #[error("database error: {0}")]
    Database(#[from] db::RepositoryError),

    /// Classification failed.
    #[error("classification failed: {0}")]
    Classification(String),
}

/// Re-run the full analysis pipeline on an email.
///
/// Loads the email from the database, enriches it with contact graph and RAG
/// context, classifies it with Claude, saves the result, updates the contact
/// graph, and stores the embedding. Does **not** route the email (no Slack
/// notifications, no M365 archive moves, no draft responses).
///
/// Call [`reset_analysis`](admin DB) before this to clear previous results
/// and roll back graph mutations.
///
/// # Errors
///
/// Returns `ReanalyzeError` if a database or classification operation fails.
pub async fn reanalyze_email(
    pool: &PgPool,
    claude: &ClaudeClient,
    embedding_client: Option<&EmbeddingClient>,
    email_id: i32,
) -> Result<(), ReanalyzeError> {
    let email = inbound_email::get_for_analysis(pool, email_id).await?;

    // Load thread context
    let thread_messages = inbound_email::get_thread_context(pool, &email.conversation_id, email_id)
        .await
        .unwrap_or_default();

    let thread_context: Vec<classifier::ThreadMessage> = thread_messages
        .into_iter()
        .map(|m| classifier::ThreadMessage {
            from: m.from_address,
            body_preview: m.body_preview,
        })
        .collect();

    // Enrich: contact graph sender context
    let sender_context = enrich_sender_context(pool, &email.from_address).await;

    // Enrich: RAG similar past emails
    let (embedding, rag_context) = retrieve_rag_context(
        embedding_client,
        pool,
        &email.subject,
        &email.body_text,
        email_id,
    )
    .await;

    // Classify
    let context = EmailContext {
        from_address: email.from_address.clone(),
        from_name: email.from_name.clone(),
        subject: email.subject.clone(),
        body: email.body_text.clone(),
        thread_context,
        sender_context,
        rag_context,
    };

    let classification = classifier::classify_email(claude, pool, &context)
        .await
        .map_err(|e| ReanalyzeError::Classification(e.to_string()))?;

    // Save classification
    inbound_email::save_classification(pool, email_id, &classification).await?;

    // Update status to classified
    inbound_email::update_status(pool, email_id, EmailStatus::Classified, None).await?;

    // Save embedding if available
    if let Some(ref emb) = embedding
        && let Err(e) = inbound_email::save_embedding(pool, email_id, emb).await
    {
        warn!(email_id, error = %e, "failed to save email embedding");
    }

    // LLM-driven graph update (best-effort)
    match graph_updater::extract_graph_updates(claude, &context, &classification).await {
        Ok(Some(updates)) => match graph_updater::apply_graph_updates(pool, &updates).await {
            Ok(contributions) => {
                if let Err(e) =
                    contact_graph::record_contributions(pool, email_id, &contributions).await
                {
                    warn!(email_id, error = %e, "failed to record graph contributions");
                }
            }
            Err(e) => warn!(email_id, error = %e, "failed to apply graph updates"),
        },
        Ok(None) => {}
        Err(e) => warn!(email_id, error = %e, "graph update extraction failed"),
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers (duplicated from triage/mod.rs to avoid exposing the full
// triage module with its routing/Slack/Shopify dependencies)
// ---------------------------------------------------------------------------

/// Strip HTML tags from a string, returning plain text.
fn strip_html(html: &str) -> String {
    if !html.contains('<') {
        return html.to_string();
    }
    html2text::from_read(html.as_bytes(), 80).unwrap_or_else(|_| html.to_string())
}

/// Look up the sender in the contact graph and return formatted context.
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

/// Retrieve similar past emails via embedding similarity search.
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

/// Format similar emails as context text for the classifier prompt.
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

//! LLM-driven contact graph updates after email classification.
//!
//! After an email is classified, a separate Claude API call analyzes the email
//! and extracts structured graph mutations (new contacts and relationships)
//! as a JSON response.

use askama::Template;
use naked_pineapple_services::claude::{
    ClaudeClient, ClaudeError, ContentBlock, Message, MessageContent,
};
use serde::Deserialize;
use sqlx::PgPool;
use tracing::{debug, instrument, warn};

use super::classifier::EmailContext;
use super::extract_json;
use super::tools::graph_update_system_prompt;
use super::truncate_with_ellipsis;
use super::types::ClassificationResult;
use crate::db::contact_graph;

/// Result of the graph update extraction.
#[derive(Debug, Deserialize)]
pub struct GraphUpdateResult {
    #[serde(default)]
    pub contacts: Vec<ContactUpdate>,
    #[serde(default)]
    pub relationships: Vec<RelationshipUpdate>,
    pub no_update_reason: Option<String>,
}

/// A contact to upsert in the graph.
#[derive(Debug, Deserialize)]
pub struct ContactUpdate {
    pub contact_type: String,
    pub name: String,
    pub email: Option<String>,
    pub domain: Option<String>,
}

/// A relationship to upsert in the graph.
#[derive(Debug, Deserialize)]
pub struct RelationshipUpdate {
    pub from_name: String,
    pub to_name: String,
    #[serde(rename = "type")]
    pub relationship_type: String,
    #[serde(default)]
    pub properties: Option<serde_json::Value>,
}

/// Allowed relationship types for validation.
const ALLOWED_RELATIONSHIP_TYPES: &[&str] = &[
    "works_at",
    "ceo_of",
    "founder_of",
    "supplies",
    "manufactures_for",
    "partners_with",
    "customer_of",
];

/// Extract graph updates from an email using Claude.
///
/// Returns `None` if the LLM decides no updates are needed.
///
/// # Errors
///
/// Returns `ClaudeError` if the API call fails or the response cannot be parsed.
#[instrument(skip(claude, email_context, classification), fields(from = %email_context.from_address))]
pub async fn extract_graph_updates(
    claude: &ClaudeClient,
    email_context: &EmailContext,
    classification: &ClassificationResult,
) -> Result<Option<GraphUpdateResult>, ClaudeError> {
    let prompt = build_graph_update_prompt(email_context, classification);

    let messages = vec![Message {
        role: "user".to_string(),
        content: MessageContent::Text(prompt),
    }];

    let system = Some(graph_update_system_prompt());

    debug!("sending graph update extraction request to Claude");
    let response = claude.chat(messages, system, None).await?;

    // Extract text content from the response
    let text: String = response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");

    if text.is_empty() {
        warn!("empty response from graph update extraction");
        return Ok(None);
    }

    let json = extract_json(&text)?;
    let result: GraphUpdateResult = serde_json::from_value(json)
        .map_err(|e| ClaudeError::Parse(format!("failed to parse graph update: {e}")))?;

    if result.no_update_reason.is_some() {
        debug!(
            reason = result.no_update_reason.as_deref().unwrap_or(""),
            "LLM decided no graph updates needed"
        );
        return Ok(None);
    }

    Ok(Some(result))
}

/// Apply graph updates to the database.
///
/// Resolves contact references by name/email/domain, creates new contacts as
/// needed, and upserts relationships. Returns the IDs of all contacts and
/// relationships that were touched so the caller can record contributions.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool, updates))]
pub async fn apply_graph_updates(
    pool: &PgPool,
    updates: &GraphUpdateResult,
) -> Result<contact_graph::GraphContributions, crate::db::RepositoryError> {
    let mut contributions = contact_graph::GraphContributions::default();

    // Upsert contacts
    for contact in &updates.contacts {
        if !is_valid_contact_type(&contact.contact_type) {
            warn!(name = %contact.name, "skipping contact with invalid type");
            continue;
        }
        let c = contact_graph::upsert_contact(
            pool,
            &contact_graph::UpsertContactParams {
                contact_type: &contact.contact_type,
                name: &contact.name,
                email: contact.email.as_deref(),
                domain: contact.domain.as_deref(),
            },
        )
        .await?;
        contributions.contact_ids.push(c.id);
    }

    // Upsert relationships
    for rel in &updates.relationships {
        if !ALLOWED_RELATIONSHIP_TYPES.contains(&rel.relationship_type.as_str()) {
            warn!(rel_type = %rel.relationship_type, "skipping unknown relationship type");
            continue;
        }

        let from = resolve_contact(pool, &rel.from_name).await?;
        let to = resolve_contact(pool, &rel.to_name).await?;

        let (Some(from_contact), Some(to_contact)) = (from, to) else {
            warn!(
                from = %rel.from_name,
                to = %rel.to_name,
                "could not resolve both contacts for relationship"
            );
            continue;
        };

        if from_contact.id == to_contact.id {
            warn!("skipping self-relationship");
            continue;
        }

        contributions.contact_ids.push(from_contact.id);
        contributions.contact_ids.push(to_contact.id);

        let rel_id = contact_graph::upsert_relationship(
            pool,
            &contact_graph::UpsertRelationshipParams {
                from_id: from_contact.id,
                to_id: to_contact.id,
                relationship_type: rel.relationship_type.clone(),
                properties: rel
                    .properties
                    .clone()
                    .unwrap_or_else(|| serde_json::json!({})),
            },
        )
        .await?;
        contributions.relationship_ids.push(rel_id);
    }

    // Deduplicate IDs (contacts may appear in both the contacts list and
    // as resolved endpoints of relationships).
    contributions.contact_ids.sort_unstable();
    contributions.contact_ids.dedup();

    Ok(contributions)
}

/// Resolve a contact reference by name, creating if necessary.
async fn resolve_contact(
    pool: &PgPool,
    name: &str,
) -> Result<Option<contact_graph::Contact>, crate::db::RepositoryError> {
    // Try exact name match first
    if let Some(contact) = contact_graph::find_by_name(pool, name).await? {
        return Ok(Some(contact));
    }

    // Try search
    let results = contact_graph::search(pool, name).await?;
    if let Some(contact) = results.into_iter().next() {
        return Ok(Some(contact));
    }

    // Create as organization (most common for new entities from emails)
    let contact = contact_graph::upsert_contact(
        pool,
        &contact_graph::UpsertContactParams {
            contact_type: "organization",
            name,
            email: None,
            domain: None,
        },
    )
    .await?;

    Ok(Some(contact))
}

fn is_valid_contact_type(t: &str) -> bool {
    matches!(t, "person" | "organization")
}

/// Askama template for the graph update extraction user prompt.
#[derive(Template)]
#[template(path = "prompts/graph_update.txt")]
struct GraphUpdatePrompt<'a> {
    classification: String,
    confidence: String,
    from_address: &'a str,
    from_name: Option<&'a str>,
    subject: &'a str,
    body: String,
}

/// Build the user prompt for graph update extraction.
fn build_graph_update_prompt(
    context: &EmailContext,
    classification: &ClassificationResult,
) -> String {
    let template = GraphUpdatePrompt {
        classification: classification.classification.to_string(),
        confidence: format!("{:.0}", classification.confidence * 100.0),
        from_address: &context.from_address,
        from_name: context.from_name.as_deref(),
        subject: &context.subject,
        body: truncate_with_ellipsis(&context.body, 1500),
    };

    template.render().expect("graph update prompt template")
}

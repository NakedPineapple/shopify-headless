//! Database operations for the `inbound_email` table.
//!
//! CRUD operations for storing and updating inbound emails processed
//! by the triage pipeline.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::instrument;

use super::{RepositoryError, to_time_offset};
use crate::triage::types::{ClassificationResult, EmailStatus};

/// Parameters for inserting a new inbound email.
pub struct InsertParams<'a> {
    pub m365_message_id: &'a str,
    pub conversation_id: &'a str,
    pub mailbox: &'a str,
    pub from_address: &'a str,
    pub from_name: Option<&'a str>,
    pub to_addresses: &'a serde_json::Value,
    pub subject: &'a str,
    pub body_preview: &'a str,
    pub body_text: &'a str,
    pub received_at: DateTime<Utc>,
    pub folder: Option<&'a str>,
    pub is_read: bool,
}

/// A prior message in a thread, for providing context to the classifier.
pub struct ThreadContextRow {
    pub from_address: String,
    pub body_preview: String,
}

/// Core email fields needed for the analysis pipeline.
pub struct EmailForAnalysis {
    pub conversation_id: String,
    pub from_address: String,
    pub from_name: Option<String>,
    pub subject: String,
    pub body_text: String,
}

/// Check if an email with this M365 message ID already exists.
///
/// Returns `Existing::No` if it doesn't exist, `Existing::Processed(id)` if
/// it has already been classified, or `Existing::Pending(id)` if it was reset
/// for re-analysis.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn check_existing(
    pool: &PgPool,
    m365_message_id: &str,
) -> Result<Existing, RepositoryError> {
    let row = sqlx::query!(
        r#"
        SELECT id, status
        FROM admin.inbound_email
        WHERE m365_message_id = $1
        "#,
        m365_message_id
    )
    .fetch_optional(pool)
    .await?;

    match row {
        None => Ok(Existing::No),
        Some(r) if r.status == "pending" => Ok(Existing::Pending(r.id)),
        Some(_) => Ok(Existing::Processed),
    }
}

/// Result of checking whether an email already exists in the database.
pub enum Existing {
    /// No record exists — this is a new email.
    No,
    /// Record exists but has been reset to "pending" for re-analysis.
    Pending(i32),
    /// Record exists and has already been processed.
    Processed,
}

/// Insert a new inbound email record. Returns the new row's ID.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool, params), fields(m365_id = %params.m365_message_id))]
pub async fn insert(pool: &PgPool, params: &InsertParams<'_>) -> Result<i32, RepositoryError> {
    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO admin.inbound_email (
            m365_message_id, conversation_id, mailbox,
            from_address, from_name, to_addresses,
            subject, body_preview, body_text,
            received_at, status, folder, is_read
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        RETURNING id
        "#,
        params.m365_message_id,
        params.conversation_id,
        params.mailbox,
        params.from_address,
        params.from_name,
        params.to_addresses,
        params.subject,
        params.body_preview,
        params.body_text,
        to_time_offset(params.received_at),
        EmailStatus::Pending.as_str(),
        params.folder,
        params.is_read,
    )
    .fetch_one(pool)
    .await?;

    Ok(id)
}

/// Fetch core fields needed for the analysis pipeline.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool), fields(%email_id))]
pub async fn get_for_analysis(
    pool: &PgPool,
    email_id: i32,
) -> Result<EmailForAnalysis, RepositoryError> {
    let row = sqlx::query_as!(
        EmailForAnalysis,
        r#"
        SELECT
            conversation_id,
            from_address,
            from_name,
            subject,
            COALESCE(body_text, '') AS "body_text!"
        FROM admin.inbound_email
        WHERE id = $1
        "#,
        email_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Save AI classification results to an inbound email record.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool, result), fields(%email_id))]
pub async fn save_classification(
    pool: &PgPool,
    email_id: i32,
    result: &ClassificationResult,
) -> Result<(), RepositoryError> {
    let classification_str = serde_json::to_value(result.classification)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| format!("{:?}", result.classification));

    // The REAL column stores f32, and confidence is always 0.0-1.0 so no precision loss.
    #[allow(clippy::cast_possible_truncation)]
    let confidence = result.confidence as f32;

    sqlx::query!(
        r#"
        UPDATE admin.inbound_email
        SET classification = $1,
            sub_category = $2,
            confidence = $3,
            reasoning = $4,
            status = $5,
            updated_at = NOW()
        WHERE id = $6
        "#,
        classification_str,
        result.sub_category,
        confidence,
        result.reasoning,
        EmailStatus::Classified.as_str(),
        email_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Update the status and `routed_to` fields.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool), fields(%email_id, %status))]
pub async fn update_status(
    pool: &PgPool,
    email_id: i32,
    status: EmailStatus,
    routed_to: Option<&str>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.inbound_email
        SET status = $1, routed_to = $2, updated_at = NOW()
        WHERE id = $3
        "#,
        status.as_str(),
        routed_to,
        email_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Save a draft response for Slack review.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool, draft), fields(%email_id))]
pub async fn save_response_draft(
    pool: &PgPool,
    email_id: i32,
    draft: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.inbound_email
        SET response_draft = $1, updated_at = NOW()
        WHERE id = $2
        "#,
        draft,
        email_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Mark a response as approved and record sent timestamp.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool), fields(%email_id))]
pub async fn mark_response_sent(pool: &PgPool, email_id: i32) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.inbound_email
        SET response_approved = TRUE,
            response_sent_at = NOW(),
            status = $1,
            updated_at = NOW()
        WHERE id = $2
        "#,
        EmailStatus::Responded.as_str(),
        email_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Mark a review as rejected (reviewed but not sent).
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool), fields(%email_id))]
pub async fn mark_review_rejected(
    pool: &PgPool,
    email_id: i32,
    reviewer: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.inbound_email
        SET response_approved = FALSE,
            reviewed_by = $1,
            reviewed_at = NOW(),
            status = $2,
            updated_at = NOW()
        WHERE id = $3
        "#,
        reviewer,
        EmailStatus::Routed.as_str(),
        email_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Set an error message on an email record.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool, error_msg), fields(%email_id))]
pub async fn set_error(
    pool: &PgPool,
    email_id: i32,
    error_msg: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.inbound_email
        SET error = $1, updated_at = NOW()
        WHERE id = $2
        "#,
        error_msg,
        email_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Check if a conversation thread already has a sent auto-response.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn has_sent_response_in_thread(
    pool: &PgPool,
    conversation_id: &str,
) -> Result<bool, RepositoryError> {
    let exists = sqlx::query_scalar!(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM admin.inbound_email
            WHERE conversation_id = $1
              AND response_sent_at IS NOT NULL
        ) AS "exists!"
        "#,
        conversation_id
    )
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

/// Get prior messages in a thread for classification context.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn get_thread_context(
    pool: &PgPool,
    conversation_id: &str,
    exclude_id: i32,
) -> Result<Vec<ThreadContextRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        ThreadContextRow,
        r#"
        SELECT from_address, COALESCE(body_preview, '') AS "body_preview!"
        FROM admin.inbound_email
        WHERE conversation_id = $1 AND id != $2
        ORDER BY received_at DESC
        LIMIT 5
        "#,
        conversation_id,
        exclude_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// A similar past email found via embedding similarity search.
pub struct SimilarEmail {
    pub id: i32,
    pub from_address: String,
    pub subject: String,
    pub body_preview: String,
    pub classification: Option<String>,
    pub similarity: f64,
}

/// Save an embedding vector for an inbound email.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool, embedding), fields(%email_id))]
pub async fn save_embedding(
    pool: &PgPool,
    email_id: i32,
    embedding: &[f32],
) -> Result<(), RepositoryError> {
    let embedding_str = format_embedding(embedding);
    sqlx::query!(
        r#"
        UPDATE admin.inbound_email
        SET embedding = $1::text::vector,
            updated_at = NOW()
        WHERE id = $2
        "#,
        embedding_str,
        email_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Search for similar past emails by cosine similarity.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool, embedding))]
pub async fn search_similar(
    pool: &PgPool,
    embedding: &[f32],
    limit: i64,
    exclude_id: Option<i32>,
) -> Result<Vec<SimilarEmail>, RepositoryError> {
    let embedding_str = format_embedding(embedding);
    let rows = sqlx::query_as!(
        SimilarEmailRow,
        r#"
        SELECT
            id,
            from_address,
            subject,
            COALESCE(body_preview, '') AS "body_preview!",
            classification,
            1 - (embedding <=> $1::text::vector) AS "similarity!"
        FROM admin.inbound_email
        WHERE embedding IS NOT NULL
          AND ($3::int IS NULL OR id != $3)
        ORDER BY embedding <=> $1::text::vector
        LIMIT $2
        "#,
        embedding_str,
        limit,
        exclude_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

/// Format an embedding vector as a pgvector literal string.
fn format_embedding(embedding: &[f32]) -> String {
    let values: Vec<String> = embedding.iter().map(ToString::to_string).collect();
    format!("[{}]", values.join(","))
}

#[derive(Debug)]
struct SimilarEmailRow {
    id: i32,
    from_address: String,
    subject: String,
    body_preview: String,
    classification: Option<String>,
    similarity: f64,
}

impl From<SimilarEmailRow> for SimilarEmail {
    fn from(row: SimilarEmailRow) -> Self {
        Self {
            id: row.id,
            from_address: row.from_address,
            subject: row.subject,
            body_preview: row.body_preview,
            classification: row.classification,
            similarity: row.similarity,
        }
    }
}

/// Info for Slack review actions (approve/reject).
pub struct EmailReviewInfo {
    pub m365_message_id: String,
    pub mailbox: String,
    pub from_address: String,
    pub from_name: Option<String>,
    pub subject: String,
    pub classification: Option<String>,
    pub reasoning: Option<String>,
    pub response_draft: Option<String>,
}

/// Fetch review info for an email by ID (used by Slack webhook handlers).
///
/// # Errors
///
/// Returns `RepositoryError::NotFound` if the entity does not exist, or
/// `RepositoryError::Database` if the query fails.
#[instrument(skip(pool), fields(%email_id))]
pub async fn get_review_info(
    pool: &PgPool,
    email_id: i32,
) -> Result<EmailReviewInfo, RepositoryError> {
    let row = sqlx::query_as!(
        EmailReviewInfo,
        r#"
        SELECT m365_message_id, mailbox, from_address, from_name,
               subject, classification, reasoning, response_draft
        FROM admin.inbound_email
        WHERE id = $1
        "#,
        email_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(RepositoryError::NotFound)?;

    Ok(row)
}

/// Mark a review as approved by a human (without sending a reply).
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool), fields(%email_id))]
pub async fn mark_review_approved(
    pool: &PgPool,
    email_id: i32,
    reviewer: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.inbound_email
        SET response_approved = TRUE,
            reviewed_by = $1,
            reviewed_at = NOW(),
            status = $2,
            updated_at = NOW()
        WHERE id = $3
        "#,
        reviewer,
        EmailStatus::Routed.as_str(),
        email_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

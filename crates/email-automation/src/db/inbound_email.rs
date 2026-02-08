//! Database operations for the `inbound_email` table.
//!
//! CRUD operations for storing and updating inbound emails processed
//! by the triage pipeline.

use chrono::{DateTime, Datelike, Timelike, Utc};
use sqlx::PgPool;
use tracing::instrument;

use super::RepositoryError;
use crate::triage::types::{ClassificationResult, EmailStatus};

/// Convert chrono `DateTime<Utc>` to `time::OffsetDateTime` for `SQLx` compatibility.
///
/// `SQLx` exhibits asymmetric behavior when both `chrono` and `time` crates are in the
/// dependency graph: reads work with chrono via type annotations, but writes (bind
/// parameters) expect `time` types. See `crates/admin/src/db/manufacturing.rs` for the
/// equivalent `DATE` conversion and full explanation.
fn to_time_offset_date_time(dt: DateTime<Utc>) -> time::OffsetDateTime {
    let date = time::Date::from_calendar_date(
        dt.year(),
        time::Month::try_from(u8::try_from(dt.month()).expect("month in range"))
            .expect("valid month"),
        u8::try_from(dt.day()).expect("day in range"),
    )
    .expect("valid date");
    let time = time::Time::from_hms_nano(
        u8::try_from(dt.hour()).expect("hour in range"),
        u8::try_from(dt.minute()).expect("minute in range"),
        u8::try_from(dt.second()).expect("second in range"),
        dt.timestamp_subsec_nanos(),
    )
    .expect("valid time");
    time::OffsetDateTime::new_utc(date, time)
}

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
}

/// A prior message in a thread, for providing context to the classifier.
pub struct ThreadContextRow {
    pub from_address: String,
    pub body_preview: String,
}

/// Check if an email with this M365 message ID already exists.
#[instrument(skip(pool))]
pub async fn exists_by_m365_id(
    pool: &PgPool,
    m365_message_id: &str,
) -> Result<bool, RepositoryError> {
    let exists = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM admin.inbound_email WHERE m365_message_id = $1) AS \"exists!\"",
        m365_message_id
    )
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

/// Insert a new inbound email record. Returns the new row's ID.
#[instrument(skip(pool, params), fields(m365_id = %params.m365_message_id))]
pub async fn insert(pool: &PgPool, params: &InsertParams<'_>) -> Result<i32, RepositoryError> {
    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO admin.inbound_email (
            m365_message_id, conversation_id, mailbox,
            from_address, from_name, to_addresses,
            subject, body_preview, body_text,
            received_at, status
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
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
        to_time_offset_date_time(params.received_at),
        EmailStatus::Pending.as_str(),
    )
    .fetch_one(pool)
    .await?;

    Ok(id)
}

/// Save AI classification results to an inbound email record.
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

/// Get the M365 message ID and mailbox for an email (for sending replies).
pub struct EmailReplyInfo {
    pub m365_message_id: String,
    pub mailbox: String,
    pub from_address: String,
    pub response_draft: Option<String>,
}

/// Fetch reply info for an email by ID.
#[instrument(skip(pool), fields(%email_id))]
pub async fn get_reply_info(
    pool: &PgPool,
    email_id: i32,
) -> Result<EmailReplyInfo, RepositoryError> {
    let row = sqlx::query_as!(
        EmailReplyInfo,
        r#"
        SELECT m365_message_id, mailbox, from_address, response_draft
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

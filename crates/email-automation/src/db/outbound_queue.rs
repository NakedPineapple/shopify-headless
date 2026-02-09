//! Database operations for the `outbound_email_queue` table.
//!
//! Manages the queue of transactional emails waiting to be sent.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::instrument;

use super::{RepositoryError, to_time_offset};

/// A queued outbound email ready for sending.
pub struct QueuedEmail {
    pub id: i64,
    pub email_type: String,
    pub to_address: String,
    pub to_name: Option<String>,
    pub subject: String,
    pub body_html: String,
    pub attempts: i32,
    pub max_attempts: i32,
}

/// Parameters for inserting a new outbound email.
pub struct EnqueueParams<'a> {
    pub email_type: &'a str,
    pub to_address: &'a str,
    pub to_name: Option<&'a str>,
    pub subject: &'a str,
    pub body_html: &'a str,
    pub body_text: &'a str,
    pub reference_id: Option<&'a str>,
    pub reference_type: Option<&'a str>,
    pub scheduled_for: Option<DateTime<Utc>>,
}

/// Check whether an email with the given type and reference already exists in the queue.
#[instrument(skip(pool))]
pub async fn exists(
    pool: &PgPool,
    email_type: &str,
    reference_id: &str,
) -> Result<bool, RepositoryError> {
    let exists = sqlx::query_scalar!(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM admin.outbound_email_queue
            WHERE email_type = $1 AND reference_id = $2
        ) AS "exists!"
        "#,
        email_type,
        reference_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

/// Insert a new email into the outbound queue. Returns the queue row ID.
#[instrument(skip(pool, params), fields(email_type = %params.email_type, to = %params.to_address))]
pub async fn enqueue(pool: &PgPool, params: &EnqueueParams<'_>) -> Result<i64, RepositoryError> {
    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO admin.outbound_email_queue (
            email_type, to_address, to_name,
            subject, body_html, body_text,
            reference_id, reference_type, scheduled_for
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, COALESCE($9, NOW()))
        RETURNING id
        "#,
        params.email_type,
        params.to_address,
        params.to_name,
        params.subject,
        params.body_html,
        params.body_text,
        params.reference_id,
        params.reference_type,
        params.scheduled_for.map(to_time_offset),
    )
    .fetch_one(pool)
    .await?;

    Ok(id)
}

/// Fetch emails that are ready to send (queued and `scheduled_for` <= now).
#[instrument(skip(pool))]
pub async fn fetch_ready(pool: &PgPool, limit: i64) -> Result<Vec<QueuedEmail>, RepositoryError> {
    let rows = sqlx::query_as!(
        QueuedEmail,
        r#"
        SELECT id, email_type, to_address, to_name,
               subject, body_html,
               attempts, max_attempts
        FROM admin.outbound_email_queue
        WHERE status = 'queued'
          AND scheduled_for <= NOW()
          AND attempts < max_attempts
        ORDER BY scheduled_for ASC
        LIMIT $1
        "#,
        limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Mark an email as successfully sent.
#[instrument(skip(pool), fields(%email_id))]
pub async fn mark_sent(pool: &PgPool, email_id: i64) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.outbound_email_queue
        SET status = 'sent', sent_at = NOW(), last_attempt_at = NOW()
        WHERE id = $1
        "#,
        email_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Record a send failure and increment the attempt counter.
/// If max attempts reached, marks status as `failed`.
#[instrument(skip(pool, error_msg), fields(%email_id))]
pub async fn mark_failed(
    pool: &PgPool,
    email_id: i64,
    error_msg: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.outbound_email_queue
        SET attempts = attempts + 1,
            last_attempt_at = NOW(),
            error_message = $1,
            status = CASE
                WHEN attempts + 1 >= max_attempts THEN 'failed'
                ELSE 'queued'
            END
        WHERE id = $2
        "#,
        error_msg,
        email_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

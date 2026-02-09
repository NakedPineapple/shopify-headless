//! Database operations for the `outbound_email_queue` table.
//!
//! Used for deduplication tracking of Klaviyo events and SMTP alerts.

use sqlx::PgPool;
use tracing::instrument;

use super::RepositoryError;

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

/// Record that a Klaviyo event was fired, for deduplication.
///
/// Inserts a minimal row so [`exists`] prevents duplicate events on subsequent polls.
#[instrument(skip(pool))]
pub async fn record_tracked(
    pool: &PgPool,
    email_type: &str,
    reference_id: &str,
    reference_type: &str,
) -> Result<i64, RepositoryError> {
    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO admin.outbound_email_queue (
            email_type, to_address, subject,
            body_html, body_text,
            status, sent_at,
            reference_id, reference_type
        )
        VALUES ($1, 'klaviyo', '', '', '', 'sent', NOW(), $2, $3)
        RETURNING id
        "#,
        email_type,
        reference_id,
        reference_type,
    )
    .fetch_one(pool)
    .await?;

    Ok(id)
}

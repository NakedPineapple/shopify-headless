//! Database operations for the `email_sync_state` table.
//!
//! Tracks per-mailbox high water marks for incremental email sync.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::instrument;

use super::{RepositoryError, to_time_offset};

/// Get the high water mark for a mailbox.
///
/// Returns `None` if no sync state exists yet (first sync).
#[instrument(skip(pool))]
pub async fn get_high_water_mark(
    pool: &PgPool,
    mailbox: &str,
) -> Result<Option<DateTime<Utc>>, RepositoryError> {
    let row = sqlx::query_scalar!(
        r#"
        SELECT high_water_mark as "high_water_mark: DateTime<Utc>"
        FROM admin.email_sync_state
        WHERE mailbox = $1
        "#,
        mailbox
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Upsert the high water mark for a mailbox.
///
/// Creates the row on first call, updates on subsequent calls.
#[instrument(skip(pool))]
pub async fn upsert_high_water_mark(
    pool: &PgPool,
    mailbox: &str,
    timestamp: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        INSERT INTO admin.email_sync_state (mailbox, high_water_mark)
        VALUES ($1, $2)
        ON CONFLICT (mailbox)
        DO UPDATE SET high_water_mark = $2
        "#,
        mailbox,
        to_time_offset(timestamp),
    )
    .execute(pool)
    .await?;

    Ok(())
}

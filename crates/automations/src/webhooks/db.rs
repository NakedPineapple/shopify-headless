//! Database operations for webhook event ingestion.
//!
//! These queries run against the restricted-privilege pool and can only
//! INSERT/SELECT on `admin.webhook_event`.

use sqlx::PgPool;
use tracing::instrument;

/// Insert a webhook event, ignoring duplicates based on `(source, external_id)`.
///
/// Returns `true` if a new row was inserted, `false` if it was a duplicate.
#[instrument(skip(pool, payload))]
pub async fn insert_event(
    pool: &PgPool,
    source: &str,
    event_type: &str,
    external_id: Option<&str>,
    payload: &serde_json::Value,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        INSERT INTO admin.webhook_event (source, event_type, external_id, payload)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (source, external_id)
            WHERE external_id IS NOT NULL
        DO NOTHING
        "#,
        source,
        event_type,
        external_id,
        payload,
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

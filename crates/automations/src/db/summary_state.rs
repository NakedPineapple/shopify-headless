//! Tracks when summary email workflows last ran.
//!
//! Uses `admin.summary_email_state` to persist last-run timestamps across
//! restarts, enabling wall-clock scheduling (e.g., "daily at 7 AM").

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::instrument;

use super::{RepositoryError, to_time_offset};

/// Get the last run time for a workflow.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
#[instrument(skip(pool), level = "debug")]
pub async fn get_last_run(
    pool: &PgPool,
    workflow_name: &str,
) -> Result<Option<DateTime<Utc>>, RepositoryError> {
    let row = sqlx::query_scalar!(
        r#"
        SELECT last_run_at as "last_run_at: DateTime<Utc>"
        FROM admin.summary_email_state
        WHERE workflow_name = $1
        "#,
        workflow_name,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Record that a workflow just ran.
///
/// Uses `INSERT ... ON CONFLICT` to handle both first-run and subsequent runs.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
#[instrument(skip(pool), level = "debug")]
pub async fn record_run(
    pool: &PgPool,
    workflow_name: &str,
    ran_at: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    let ran_at_time = to_time_offset(ran_at);
    sqlx::query!(
        r#"
        INSERT INTO admin.summary_email_state (workflow_name, last_run_at)
        VALUES ($1, $2)
        ON CONFLICT (workflow_name) DO UPDATE
            SET last_run_at = EXCLUDED.last_run_at,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
        "#,
        workflow_name,
        ran_at_time,
    )
    .execute(pool)
    .await?;

    Ok(())
}

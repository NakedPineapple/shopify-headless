//! Database operations for the `automation_log` table.
//!
//! Tracks workflow executions with timing, item counts, and metadata.
//! Also supports deduplication queries (e.g., suppress low stock alerts
//! for the same product within 24 hours).

use sqlx::PgPool;
use tracing::instrument;

use super::RepositoryError;

/// Start a new automation run. Returns the log entry ID.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn start_run(pool: &PgPool, workflow: &str) -> Result<i64, RepositoryError> {
    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO admin.automation_log (workflow, status)
        VALUES ($1, 'running')
        RETURNING id
        "#,
        workflow,
    )
    .fetch_one(pool)
    .await?;

    Ok(id)
}

/// Mark a run as completed successfully.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool), fields(%log_id))]
pub async fn complete_run(
    pool: &PgPool,
    log_id: i64,
    items_processed: i32,
    items_actioned: i32,
    metadata: Option<&serde_json::Value>,
    duration_ms: i64,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.automation_log
        SET status = 'completed',
            items_processed = $1,
            items_actioned = $2,
            metadata = $3,
            completed_at = NOW(),
            duration_ms = $4
        WHERE id = $5
        "#,
        items_processed,
        items_actioned,
        metadata,
        duration_ms,
        log_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Mark a run as failed.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool, error_msg), fields(%log_id))]
pub async fn fail_run(
    pool: &PgPool,
    log_id: i64,
    error_msg: &str,
    duration_ms: i64,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.automation_log
        SET status = 'failed',
            error = $1,
            completed_at = NOW(),
            duration_ms = $2
        WHERE id = $3
        "#,
        error_msg,
        duration_ms,
        log_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Check if a low stock alert was already sent for a product within the
/// given number of hours. Uses the `metadata` JSONB column to match
/// on `product_id`.
///
/// # Errors
///
/// Returns `RepositoryError` if the database operation fails.
#[instrument(skip(pool))]
pub async fn has_recent_alert(
    pool: &PgPool,
    workflow: &str,
    product_id: &str,
    within_hours: i32,
) -> Result<bool, RepositoryError> {
    let filter = serde_json::json!({ "product_id": product_id });
    let exists = sqlx::query_scalar!(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM admin.automation_log
            WHERE workflow = $1
              AND status = 'completed'
              AND metadata @> $2
              AND started_at > NOW() - make_interval(hours => $3)
        ) AS "exists!"
        "#,
        workflow,
        filter,
        within_hours,
    )
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

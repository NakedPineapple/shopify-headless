//! Database operations for pending actions (Slack confirmation queue).

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::RepositoryError;

/// Status of a pending action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "action_status", rename_all = "lowercase")]
pub enum ActionStatus {
    Pending,
    Approved,
    Rejected,
    Executed,
    Failed,
    Expired,
}

/// A pending action awaiting confirmation.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PendingAction {
    /// Unique action ID.
    pub id: Uuid,
    /// Chat session this action belongs to.
    pub chat_session_id: i32,
    /// Chat message that triggered this action.
    pub chat_message_id: Option<i32>,
    /// Admin user who initiated the action.
    pub admin_user_id: i32,
    /// Tool name to execute.
    pub tool_name: String,
    /// Tool input parameters.
    pub tool_input: serde_json::Value,
    /// Current status.
    pub status: ActionStatus,
    /// Slack message timestamp (for updating).
    pub slack_message_ts: Option<String>,
    /// Slack channel ID.
    pub slack_channel_id: Option<String>,
    /// Execution result (if executed).
    pub result: Option<serde_json::Value>,
    /// Error message (if failed).
    pub error_message: Option<String>,
    /// Who approved (Slack username).
    pub approved_by: Option<String>,
    /// Who rejected (Slack username).
    pub rejected_by: Option<String>,
    /// When the action was created.
    pub created_at: DateTime<Utc>,
    /// When the action was resolved.
    pub resolved_at: Option<DateTime<Utc>>,
    /// When the action expires.
    pub expires_at: DateTime<Utc>,
}

/// Parameters for creating a pending action.
pub struct CreatePendingAction {
    /// Chat session ID.
    pub chat_session_id: i32,
    /// Chat message ID (optional).
    pub chat_message_id: Option<i32>,
    /// Admin user ID.
    pub admin_user_id: i32,
    /// Tool name.
    pub tool_name: String,
    /// Tool input.
    pub tool_input: serde_json::Value,
}

/// Create a new pending action.
///
/// # Errors
///
/// Returns error if the database insert fails.
pub async fn create_pending_action(
    pool: &PgPool,
    params: CreatePendingAction,
) -> Result<PendingAction, RepositoryError> {
    let action = sqlx::query_as!(
        PendingAction,
        r#"
        INSERT INTO admin.pending_actions (
            chat_session_id, chat_message_id, admin_user_id, tool_name, tool_input
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING
            id, chat_session_id, chat_message_id, admin_user_id,
            tool_name, tool_input, status as "status: ActionStatus",
            slack_message_ts, slack_channel_id, result, error_message,
            approved_by, rejected_by,
            created_at as "created_at: DateTime<Utc>",
            resolved_at as "resolved_at: DateTime<Utc>",
            expires_at as "expires_at: DateTime<Utc>"
        "#,
        params.chat_session_id,
        params.chat_message_id,
        params.admin_user_id,
        params.tool_name,
        params.tool_input,
    )
    .fetch_one(pool)
    .await?;

    Ok(action)
}

/// Get a pending action by ID.
///
/// # Errors
///
/// Returns error if the database query fails.
pub async fn get_pending_action(
    pool: &PgPool,
    action_id: Uuid,
) -> Result<Option<PendingAction>, RepositoryError> {
    let action = sqlx::query_as!(
        PendingAction,
        r#"
        SELECT
            id, chat_session_id, chat_message_id, admin_user_id,
            tool_name, tool_input, status as "status: ActionStatus",
            slack_message_ts, slack_channel_id, result, error_message,
            approved_by, rejected_by,
            created_at as "created_at: DateTime<Utc>",
            resolved_at as "resolved_at: DateTime<Utc>",
            expires_at as "expires_at: DateTime<Utc>"
        FROM admin.pending_actions
        WHERE id = $1
        "#,
        action_id,
    )
    .fetch_optional(pool)
    .await?;

    Ok(action)
}

/// Update the Slack message information for an action.
///
/// # Errors
///
/// Returns error if the database update fails.
pub async fn update_slack_info(
    pool: &PgPool,
    action_id: Uuid,
    message_ts: &str,
    channel_id: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.pending_actions
        SET slack_message_ts = $2, slack_channel_id = $3
        WHERE id = $1
        "#,
        action_id,
        message_ts,
        channel_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Approve a pending action.
///
/// # Errors
///
/// Returns error if the database update fails.
pub async fn approve_action(
    pool: &PgPool,
    action_id: Uuid,
    approved_by: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.pending_actions
        SET status = 'approved', approved_by = $2, resolved_at = NOW()
        WHERE id = $1 AND status = 'pending'
        "#,
        action_id,
        approved_by,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Reject a pending action.
///
/// # Errors
///
/// Returns error if the database update fails.
pub async fn reject_action(
    pool: &PgPool,
    action_id: Uuid,
    rejected_by: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.pending_actions
        SET status = 'rejected', rejected_by = $2, resolved_at = NOW()
        WHERE id = $1 AND status = 'pending'
        "#,
        action_id,
        rejected_by,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Mark an action as executed with result.
///
/// # Errors
///
/// Returns error if the database update fails.
pub async fn mark_executed(
    pool: &PgPool,
    action_id: Uuid,
    result: &serde_json::Value,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.pending_actions
        SET status = 'executed', result = $2
        WHERE id = $1
        "#,
        action_id,
        result,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Mark an action as failed with error.
///
/// # Errors
///
/// Returns error if the database update fails.
pub async fn mark_failed(
    pool: &PgPool,
    action_id: Uuid,
    error_message: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.pending_actions
        SET status = 'failed', error_message = $2
        WHERE id = $1
        "#,
        action_id,
        error_message,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Get pending actions that have expired.
///
/// Returns actions that are pending but past their expiry time.
/// Call this before `expire_stale_actions` to get the list for notifications.
///
/// # Errors
///
/// Returns error if the database query fails.
pub async fn get_expiring_actions(pool: &PgPool) -> Result<Vec<PendingAction>, RepositoryError> {
    let actions = sqlx::query_as::<_, PendingAction>(
        r"
        SELECT
            id,
            chat_session_id,
            chat_message_id,
            admin_user_id,
            tool_name,
            tool_input,
            status,
            slack_message_ts,
            slack_channel_id,
            result,
            error_message,
            approved_by,
            rejected_by,
            created_at,
            resolved_at,
            expires_at
        FROM admin.pending_actions
        WHERE status = 'pending' AND expires_at < NOW()
        ",
    )
    .fetch_all(pool)
    .await?;

    Ok(actions)
}

/// Expire stale pending actions.
///
/// Returns the number of actions that were expired.
///
/// # Errors
///
/// Returns error if the database update fails.
pub async fn expire_stale_actions(pool: &PgPool) -> Result<u64, RepositoryError> {
    let result = sqlx::query!(
        r#"
        UPDATE admin.pending_actions
        SET status = 'expired', resolved_at = NOW()
        WHERE status = 'pending' AND expires_at < NOW()
        "#,
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Get pending actions for a chat session.
///
/// # Errors
///
/// Returns error if the database query fails.
pub async fn get_pending_actions_for_session(
    pool: &PgPool,
    chat_session_id: i32,
) -> Result<Vec<PendingAction>, RepositoryError> {
    let actions = sqlx::query_as::<_, PendingAction>(
        r"
        SELECT
            id, chat_session_id, chat_message_id, admin_user_id,
            tool_name, tool_input, status,
            slack_message_ts, slack_channel_id, result, error_message,
            approved_by, rejected_by,
            created_at, resolved_at, expires_at
        FROM admin.pending_actions
        WHERE chat_session_id = $1
        ORDER BY created_at DESC
        ",
    )
    .bind(chat_session_id)
    .fetch_all(pool)
    .await?;

    Ok(actions)
}

/// Get all pending actions for an admin user.
///
/// # Errors
///
/// Returns error if the database query fails.
pub async fn get_pending_actions_for_admin(
    pool: &PgPool,
    admin_user_id: i32,
) -> Result<Vec<PendingAction>, RepositoryError> {
    let actions = sqlx::query_as::<_, PendingAction>(
        r"
        SELECT
            id, chat_session_id, chat_message_id, admin_user_id,
            tool_name, tool_input, status,
            slack_message_ts, slack_channel_id, result, error_message,
            approved_by, rejected_by,
            created_at, resolved_at, expires_at
        FROM admin.pending_actions
        WHERE admin_user_id = $1 AND status = 'pending'
        ORDER BY created_at DESC
        ",
    )
    .bind(admin_user_id)
    .fetch_all(pool)
    .await?;

    Ok(actions)
}

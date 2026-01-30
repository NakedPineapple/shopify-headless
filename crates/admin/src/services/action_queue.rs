//! Action queue service for managing pending write operations.
//!
//! This service orchestrates the Slack confirmation flow:
//! 1. When a write tool is requested, enqueue it as a pending action
//! 2. Send a confirmation message to Slack
//! 3. Wait for admin approval/rejection via Slack interaction
//! 4. Execute the tool if approved, or mark as rejected

use sqlx::PgPool;
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

use crate::claude::tools::{ToolExecutor, ToolResult};
use crate::db::pending_actions::{self, ActionStatus, CreatePendingAction, PendingAction};
use crate::error::AppError;
use crate::shopify::AdminClient;
use crate::slack::{
    SlackClient, build_approved_message, build_confirmation_message, build_error_message,
    build_rejected_message, build_timeout_message,
};

/// Parameters for enqueueing an action.
pub struct EnqueueParams {
    /// Chat session ID.
    pub chat_session_id: i32,
    /// Chat message ID (optional).
    pub chat_message_id: Option<i32>,
    /// Admin user ID who initiated the action.
    pub admin_user_id: i32,
    /// Admin user's display name.
    pub admin_name: String,
    /// Admin user's Slack user ID for DM notifications.
    pub admin_slack_user_id: Option<String>,
    /// Tool name to execute.
    pub tool_name: String,
    /// Tool input parameters.
    pub tool_input: serde_json::Value,
    /// Domain for the tool (used for display).
    pub domain: String,
}

/// Result of enqueueing an action.
#[derive(Debug)]
pub struct EnqueueResult {
    /// The action ID.
    pub action_id: Uuid,
    /// Tool name.
    pub tool_name: String,
    /// Whether Slack message was sent.
    pub slack_sent: bool,
}

/// Action queue service for managing pending write operations.
pub struct ActionQueueService {
    pool: PgPool,
    slack: Option<SlackClient>,
}

impl ActionQueueService {
    /// Create a new action queue service.
    #[must_use]
    pub const fn new(pool: PgPool, slack: Option<SlackClient>) -> Self {
        Self { pool, slack }
    }

    /// Enqueue a write operation for confirmation.
    ///
    /// Creates a pending action and sends a Slack confirmation message.
    ///
    /// # Errors
    ///
    /// Returns error if database insert or Slack message fails.
    #[instrument(skip(self, params), fields(tool = %params.tool_name))]
    pub async fn enqueue(&self, params: EnqueueParams) -> Result<EnqueueResult, AppError> {
        // Create pending action in database
        let action = pending_actions::create_pending_action(
            &self.pool,
            CreatePendingAction {
                chat_session_id: params.chat_session_id,
                chat_message_id: params.chat_message_id,
                admin_user_id: params.admin_user_id,
                tool_name: params.tool_name.clone(),
                tool_input: params.tool_input,
            },
        )
        .await?;

        info!(action_id = %action.id, tool = %params.tool_name, "Created pending action");

        // Send Slack confirmation if configured
        let slack_sent = if let Some(slack) = &self.slack {
            match self
                .send_slack_confirmation(
                    slack,
                    &action,
                    &params.admin_name,
                    params.admin_slack_user_id.as_deref(),
                    &params.domain,
                )
                .await
            {
                Ok(()) => true,
                Err(e) => {
                    error!(error = %e, "Failed to send Slack confirmation");
                    false
                }
            }
        } else {
            debug!("Slack not configured, skipping confirmation message");
            false
        };

        Ok(EnqueueResult {
            action_id: action.id,
            tool_name: params.tool_name,
            slack_sent,
        })
    }

    /// Send Slack confirmation message.
    ///
    /// If `admin_slack_user_id` is provided, sends a DM to that user.
    /// Otherwise falls back to the default channel.
    async fn send_slack_confirmation(
        &self,
        slack: &SlackClient,
        action: &PendingAction,
        admin_name: &str,
        admin_slack_user_id: Option<&str>,
        domain: &str,
    ) -> Result<(), AppError> {
        let blocks = build_confirmation_message(
            action.id,
            &action.tool_name,
            &action.tool_input,
            admin_name,
            domain,
        );

        // Use admin's Slack user ID for DM, or fall back to default channel
        let channel = admin_slack_user_id.unwrap_or_else(|| slack.default_channel());
        let response = slack
            .post_message(
                channel,
                blocks,
                Some(&format!("AI Action: {}", action.tool_name)),
            )
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        // Store Slack message info for later updates
        if let (Some(ts), Some(ch)) = (response.ts, response.channel) {
            pending_actions::update_slack_info(&self.pool, action.id, &ts, &ch).await?;
        }

        Ok(())
    }

    /// Handle approval from Slack.
    ///
    /// Marks the action as approved and executes the tool.
    ///
    /// # Errors
    ///
    /// Returns error if database update, tool execution, or Slack update fails.
    #[instrument(skip(self, shopify))]
    pub async fn approve(
        &self,
        action_id: Uuid,
        approved_by: &str,
        shopify: &AdminClient,
    ) -> Result<String, AppError> {
        // Get the action
        let action = pending_actions::get_pending_action(&self.pool, action_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Action not found".into()))?;

        if action.status != ActionStatus::Pending {
            return Err(AppError::BadRequest(format!(
                "Action is not pending (status: {:?})",
                action.status
            )));
        }

        // Mark as approved
        pending_actions::approve_action(&self.pool, action_id, approved_by).await?;
        info!(action_id = %action_id, approved_by = %approved_by, "Action approved");

        // Execute the tool
        let executor = ToolExecutor::new(shopify);
        let result = match executor
            .execute_confirmed(&action.tool_name, &action.tool_input)
            .await
        {
            Ok(result) => {
                let result_json = serde_json::json!({"success": true, "result": result});
                pending_actions::mark_executed(&self.pool, action_id, &result_json).await?;

                // Update Slack message
                if let Some(slack) = &self.slack {
                    self.update_slack_approved(slack, &action, approved_by, Some(&result))
                        .await;
                }

                result
            }
            Err(e) => {
                let error_msg = e.to_string();
                pending_actions::mark_failed(&self.pool, action_id, &error_msg).await?;
                error!(action_id = %action_id, error = %error_msg, "Tool execution failed");

                // Update Slack message with error
                if let Some(slack) = &self.slack {
                    self.update_slack_error(slack, &action, &error_msg).await;
                }

                return Err(AppError::Internal(error_msg));
            }
        };

        Ok(result)
    }

    /// Handle rejection from Slack.
    ///
    /// # Errors
    ///
    /// Returns error if database update fails.
    #[instrument(skip(self))]
    pub async fn reject(&self, action_id: Uuid, rejected_by: &str) -> Result<(), AppError> {
        // Get the action
        let action = pending_actions::get_pending_action(&self.pool, action_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Action not found".into()))?;

        if action.status != ActionStatus::Pending {
            return Err(AppError::BadRequest(format!(
                "Action is not pending (status: {:?})",
                action.status
            )));
        }

        // Mark as rejected
        pending_actions::reject_action(&self.pool, action_id, rejected_by).await?;
        info!(action_id = %action_id, rejected_by = %rejected_by, "Action rejected");

        // Update Slack message
        if let Some(slack) = &self.slack {
            self.update_slack_rejected(slack, &action, rejected_by)
                .await;
        }

        Ok(())
    }

    /// Get a pending action by ID.
    ///
    /// # Errors
    ///
    /// Returns error if database query fails.
    pub async fn get_action(&self, action_id: Uuid) -> Result<Option<PendingAction>, AppError> {
        Ok(pending_actions::get_pending_action(&self.pool, action_id).await?)
    }

    /// Get all pending actions for a session.
    ///
    /// # Errors
    ///
    /// Returns error if database query fails.
    pub async fn get_actions_for_session(
        &self,
        session_id: i32,
    ) -> Result<Vec<PendingAction>, AppError> {
        Ok(pending_actions::get_pending_actions_for_session(&self.pool, session_id).await?)
    }

    /// Expire stale pending actions.
    ///
    /// Gets expiring actions, updates their Slack messages, then marks them as expired.
    /// Should be called periodically (e.g., via a background task).
    ///
    /// # Errors
    ///
    /// Returns error if database update fails.
    #[instrument(skip(self))]
    pub async fn expire_stale(&self) -> Result<u64, AppError> {
        // Get actions that are about to expire (before marking them expired)
        let expiring = pending_actions::get_expiring_actions(&self.pool).await?;

        if expiring.is_empty() {
            return Ok(0);
        }

        // Update Slack messages for each expiring action
        if let Some(slack) = &self.slack {
            for action in &expiring {
                self.update_slack_timeout(slack, action).await;
            }
        }

        // Now expire them in the database
        let count = pending_actions::expire_stale_actions(&self.pool).await?;

        if count > 0 {
            info!(count = %count, "Expired stale pending actions");
        }

        Ok(count)
    }

    /// Update Slack message to show timeout.
    async fn update_slack_timeout(&self, slack: &SlackClient, action: &PendingAction) {
        if let (Some(ts), Some(channel)) = (&action.slack_message_ts, &action.slack_channel_id) {
            let blocks = build_timeout_message(&action.tool_name);
            if let Err(e) = slack.update_message(channel, ts, blocks, None).await {
                error!(error = %e, action_id = %action.id, "Failed to update Slack message for timeout");
            }
        }
    }

    /// Update Slack message to show approval.
    async fn update_slack_approved(
        &self,
        slack: &SlackClient,
        action: &PendingAction,
        approved_by: &str,
        result: Option<&str>,
    ) {
        if let (Some(ts), Some(channel)) = (&action.slack_message_ts, &action.slack_channel_id) {
            let blocks = build_approved_message(&action.tool_name, approved_by, result);
            if let Err(e) = slack.update_message(channel, ts, blocks, None).await {
                error!(error = %e, "Failed to update Slack message for approval");
            }
        }
    }

    /// Update Slack message to show rejection.
    async fn update_slack_rejected(
        &self,
        slack: &SlackClient,
        action: &PendingAction,
        rejected_by: &str,
    ) {
        if let (Some(ts), Some(channel)) = (&action.slack_message_ts, &action.slack_channel_id) {
            let blocks = build_rejected_message(&action.tool_name, rejected_by);
            if let Err(e) = slack.update_message(channel, ts, blocks, None).await {
                error!(error = %e, "Failed to update Slack message for rejection");
            }
        }
    }

    /// Update Slack message to show error.
    async fn update_slack_error(&self, slack: &SlackClient, action: &PendingAction, error: &str) {
        if let (Some(ts), Some(channel)) = (&action.slack_message_ts, &action.slack_channel_id) {
            let blocks = build_error_message(&action.tool_name, error);
            if let Err(e) = slack.update_message(channel, ts, blocks, None).await {
                error!(error = %e, "Failed to update Slack message for error");
            }
        }
    }
}

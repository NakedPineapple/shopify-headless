//! Chat domain models for Claude AI integration.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use naked_pineapple_core::{AdminUserId, ChatMessageId, ChatRole, ChatSessionId};

/// A chat session with Claude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    /// Unique session ID.
    pub id: ChatSessionId,
    /// Admin user who owns this session.
    pub admin_user_id: AdminUserId,
    /// Optional session title (auto-generated from first message).
    pub title: Option<String>,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// When the session was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A message in a chat session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Unique message ID.
    pub id: ChatMessageId,
    /// Session this message belongs to.
    pub chat_session_id: ChatSessionId,
    /// Role of the message sender.
    pub role: ChatRole,
    /// Message content (flexible JSON for tool use).
    pub content: serde_json::Value,
    /// API interaction metadata for debug panel.
    pub api_interaction: Option<ApiInteraction>,
    /// When the message was created.
    pub created_at: DateTime<Utc>,
}

/// API interaction metadata for debug panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiInteraction {
    /// Unique request ID from Claude API.
    pub request_id: Option<String>,
    /// Model used for this request.
    pub model: String,
    /// Input tokens used.
    pub input_tokens: i32,
    /// Output tokens generated.
    pub output_tokens: i32,
    /// Request duration in milliseconds.
    pub duration_ms: i64,
    /// Stop reason from Claude API.
    pub stop_reason: Option<String>,
    /// Tools that were available for this request.
    pub tools_available: Option<Vec<String>>,
    /// When this API call was made.
    pub timestamp: DateTime<Utc>,
}

/// Session-level metrics aggregated from all messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSessionMetrics {
    /// Session ID.
    pub chat_session_id: ChatSessionId,
    /// Total input tokens across all API calls.
    pub total_input_tokens: i32,
    /// Total output tokens across all API calls.
    pub total_output_tokens: i32,
    /// Total number of API calls.
    pub total_api_calls: i32,
    /// Total number of tool calls.
    pub total_tool_calls: i32,
    /// Total duration in milliseconds.
    pub total_duration_ms: i64,
    /// When metrics were created.
    pub created_at: DateTime<Utc>,
    /// When metrics were last updated.
    pub updated_at: DateTime<Utc>,
}

/// Status of a pending action awaiting Slack confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "action_status", rename_all = "lowercase")]
pub enum ActionStatus {
    Pending,
    Approved,
    Rejected,
    Executed,
    Failed,
    Expired,
}

/// A pending action awaiting Slack confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAction {
    /// Unique action ID.
    pub id: Uuid,
    /// Session where this action was requested.
    pub chat_session_id: ChatSessionId,
    /// Message that triggered this action.
    pub chat_message_id: Option<ChatMessageId>,
    /// Admin who requested this action.
    pub admin_user_id: AdminUserId,
    /// Tool name to execute.
    pub tool_name: String,
    /// Tool input parameters.
    pub tool_input: serde_json::Value,
    /// Current status.
    pub status: ActionStatus,
    /// Slack message timestamp for updates.
    pub slack_message_ts: Option<String>,
    /// Slack channel ID.
    pub slack_channel_id: Option<String>,
    /// Execution result (if executed).
    pub result: Option<serde_json::Value>,
    /// Error message (if failed).
    pub error_message: Option<String>,
    /// Who approved (Slack user ID or name).
    pub approved_by: Option<String>,
    /// Who rejected (Slack user ID or name).
    pub rejected_by: Option<String>,
    /// When action was created.
    pub created_at: DateTime<Utc>,
    /// When action was resolved.
    pub resolved_at: Option<DateTime<Utc>>,
    /// When action expires.
    pub expires_at: DateTime<Utc>,
}

/// A tool example query for embedding-based selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExampleQuery {
    /// Unique ID.
    pub id: i32,
    /// Tool name this example maps to.
    pub tool_name: String,
    /// Domain this tool belongs to.
    pub domain: String,
    /// Example user query.
    pub example_query: String,
    /// Whether this was learned from usage vs pre-seeded.
    pub is_learned: bool,
    /// How many times this example led to successful tool use.
    pub usage_count: i32,
    /// When this example was created.
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_session_serialization() {
        let session = ChatSession {
            id: ChatSessionId::new(1),
            admin_user_id: AdminUserId::new(1),
            title: Some("Test Session".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&session).expect("serialize");
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"title\":\"Test Session\""));
    }

    #[test]
    fn test_chat_message_serialization() {
        let message = ChatMessage {
            id: ChatMessageId::new(1),
            chat_session_id: ChatSessionId::new(1),
            role: ChatRole::User,
            content: serde_json::json!({"text": "Hello"}),
            api_interaction: None,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&message).expect("serialize");
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"text\":\"Hello\""));
    }

    #[test]
    fn test_action_status_serialization() {
        let status = ActionStatus::Pending;
        let json = serde_json::to_string(&status).expect("serialize");
        assert_eq!(json, "\"Pending\"");
    }
}

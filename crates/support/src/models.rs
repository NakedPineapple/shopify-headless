//! Domain models for the support system.

use chrono::{DateTime, Utc};
use naked_pineapple_core::{
    SupportConversationId, SupportConversationStatus, SupportKnowledgeId, SupportMessageId,
    SupportMessageRole, SupportTicketId,
};
use serde::{Deserialize, Serialize};

/// A customer support conversation.
#[derive(Debug, Clone, Serialize)]
pub struct SupportConversation {
    pub id: SupportConversationId,
    pub session_token: String,
    pub shopify_customer_id: Option<String>,
    pub customer_email: Option<String>,
    pub customer_name: Option<String>,
    pub status: SupportConversationStatus,
    pub assigned_admin_id: Option<i32>,
    pub escalated_at: Option<DateTime<Utc>>,
    pub escalation_reason: Option<String>,
    pub title: Option<String>,
    pub source: String,
    pub is_authenticated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub last_customer_message_at: Option<DateTime<Utc>>,
    pub last_agent_message_at: Option<DateTime<Utc>>,
}

/// A message in a support conversation.
#[derive(Debug, Clone, Serialize)]
pub struct SupportMessage {
    pub id: SupportMessageId,
    pub support_conversation_id: SupportConversationId,
    pub role: SupportMessageRole,
    pub content: serde_json::Value,
    pub api_interaction: Option<serde_json::Value>,
    pub admin_user_id: Option<i32>,
    pub created_at: DateTime<Utc>,
}

impl SupportMessage {
    /// Extract the display text from the JSON content.
    ///
    /// Messages are stored as `{"text": "..."}`. This returns the inner text,
    /// falling back to the raw JSON string if the structure is unexpected.
    #[must_use]
    pub fn content_text(&self) -> &str {
        self.content
            .get("text")
            .and_then(serde_json::Value::as_str)
            .or_else(|| self.content.as_str())
            .unwrap_or("")
    }
}

/// A support ticket linked to a conversation.
#[derive(Debug, Clone, Serialize)]
pub struct SupportTicket {
    pub id: SupportTicketId,
    pub support_conversation_id: SupportConversationId,
    pub category: Option<String>,
    pub priority: String,
    pub status: String,
    pub assigned_admin_id: Option<i32>,
    pub resolution_notes: Option<String>,
    pub slack_message_ts: Option<String>,
    pub slack_channel_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

/// A knowledge base entry for RAG retrieval.
#[derive(Debug, Clone, Serialize)]
pub struct SupportKnowledge {
    pub id: SupportKnowledgeId,
    pub title: String,
    pub content: String,
    pub category: String,
    pub is_active: bool,
    pub created_by: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Parameters for creating a new conversation.
#[derive(Debug, Deserialize)]
pub struct CreateConversationParams {
    pub session_token: String,
    pub shopify_customer_id: Option<String>,
    pub customer_email: Option<String>,
    pub customer_name: Option<String>,
    pub is_authenticated: bool,
    /// Origin of the conversation: `"chat"` (default) or `"email"`.
    pub source: Option<String>,
}

/// Parameters for creating a new message.
pub struct CreateMessageParams {
    pub support_conversation_id: SupportConversationId,
    pub role: SupportMessageRole,
    pub content: serde_json::Value,
    pub api_interaction: Option<serde_json::Value>,
    pub admin_user_id: Option<i32>,
}

/// Parameters for creating a new ticket.
pub struct CreateTicketParams {
    pub support_conversation_id: SupportConversationId,
    pub category: Option<String>,
    pub priority: String,
}

/// Parameters for creating a knowledge base entry.
pub struct CreateKnowledgeParams {
    pub title: String,
    pub content: String,
    pub category: String,
    pub embedding: Vec<f32>,
    pub created_by: Option<i32>,
}

/// Parameters for updating a knowledge base entry.
pub struct UpdateKnowledgeParams {
    pub title: String,
    pub content: String,
    pub category: String,
    pub embedding: Vec<f32>,
}

/// A knowledge base search result with similarity score.
#[derive(Debug, Clone)]
pub struct KnowledgeSearchResult {
    pub entry: SupportKnowledge,
    pub similarity: f64,
}

/// Conversation with a preview of the last message (for inbox list views).
#[derive(Debug, Clone, Serialize)]
pub struct ConversationSummary {
    pub conversation: SupportConversation,
    pub last_message_preview: Option<String>,
    pub message_count: i64,
}

/// Events emitted during streaming chat responses.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatStreamEvent {
    TextDelta { text: String },
    ToolUse { id: String, name: String },
    ToolResult { tool_use_id: String },
    MessageComplete,
    Error { message: String },
}

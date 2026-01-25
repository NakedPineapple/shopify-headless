//! Chat domain models for Claude AI integration.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    /// When the message was created.
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
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&message).expect("serialize");
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"text\":\"Hello\""));
    }
}

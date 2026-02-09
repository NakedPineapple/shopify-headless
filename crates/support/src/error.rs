//! Error types for the support system.

use thiserror::Error;

/// Errors that can occur in the support system.
#[derive(Debug, Error)]
pub enum SupportError {
    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Claude API error.
    #[error("claude error: {0}")]
    Claude(#[from] naked_pineapple_services::claude::ClaudeError),

    /// Embedding error.
    #[error("embedding error: {0}")]
    Embedding(#[from] naked_pineapple_services::openai::EmbeddingError),

    /// Conversation not found.
    #[error("conversation not found")]
    ConversationNotFound,

    /// Conversation does not belong to this session/customer.
    #[error("not authorized to access this conversation")]
    NotAuthorized,

    /// Message too long.
    #[error("message exceeds maximum length of {0} characters")]
    MessageTooLong(usize),

    /// Too many messages in conversation.
    #[error("conversation has reached the maximum of {0} messages")]
    TooManyMessages(i64),

    /// Tool execution failed.
    #[error("tool execution failed: {0}")]
    ToolExecution(String),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Authentication required for this action.
    #[error("authentication required")]
    AuthRequired,
}

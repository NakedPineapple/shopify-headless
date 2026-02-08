//! Types for the Claude API.
//!
//! These types match the Anthropic Messages API format for tool use.

use serde::{Deserialize, Serialize};

/// A message in a conversation with Claude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The role of the message sender ("user" or "assistant").
    pub role: String,
    /// The content of the message.
    pub content: MessageContent,
}

/// Content of a message - either plain text or a list of content blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Simple text content.
    Text(String),
    /// Multiple content blocks (for tool use).
    Blocks(Vec<ContentBlock>),
}

/// A content block within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    /// Text content.
    #[serde(rename = "text")]
    Text {
        /// The text content.
        text: String,
    },
    /// Tool use request from Claude.
    #[serde(rename = "tool_use")]
    ToolUse {
        /// Unique ID for this tool use.
        id: String,
        /// Name of the tool to use.
        name: String,
        /// Input parameters for the tool.
        input: serde_json::Value,
    },
    /// Result of a tool invocation.
    #[serde(rename = "tool_result")]
    ToolResult {
        /// ID of the tool use this is responding to.
        tool_use_id: String,
        /// Result content from the tool.
        content: String,
        /// Whether the tool execution failed.
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// A tool definition for Claude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// Name of the tool.
    pub name: String,
    /// Description of what the tool does.
    pub description: String,
    /// JSON Schema for the tool's input parameters.
    pub input_schema: serde_json::Value,
    /// Domain the tool belongs to (internal, not sent to Claude).
    #[serde(skip)]
    pub domain: Option<String>,
    /// Whether this tool requires confirmation before execution (internal).
    #[serde(skip)]
    pub requires_confirmation: bool,
}

/// Request body for the Claude Messages API.
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    /// Model to use (e.g., "claude-sonnet-4-20250514").
    pub model: String,
    /// Maximum tokens to generate.
    pub max_tokens: u32,
    /// Conversation messages.
    pub messages: Vec<Message>,
    /// System prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Available tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    /// Whether to stream the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// Response from the Claude Messages API (non-streaming).
#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    /// Unique response ID.
    pub id: String,
    /// Model that generated the response.
    pub model: String,
    /// Reason the response stopped.
    pub stop_reason: Option<StopReason>,
    /// Response content blocks.
    pub content: Vec<ContentBlock>,
    /// Token usage information.
    pub usage: Usage,
}

/// Reason the model stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Natural end of response.
    EndTurn,
    /// Max tokens reached.
    MaxTokens,
    /// Stop sequence encountered.
    StopSequence,
    /// Tool use requested.
    ToolUse,
}

/// Token usage information.
#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    /// Number of input tokens.
    pub input_tokens: u32,
    /// Number of output tokens.
    pub output_tokens: u32,
}

// =============================================================================
// Streaming Types
// =============================================================================

/// Server-Sent Event types from Claude streaming API.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    /// Start of a message.
    #[serde(rename = "message_start")]
    MessageStart {
        /// The initial message object.
        message: StreamMessage,
    },
    /// Start of a content block.
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        /// Index of the content block.
        index: usize,
        /// The content block.
        content_block: ContentBlockStart,
    },
    /// Delta update for a content block.
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        /// Index of the content block.
        index: usize,
        /// The delta update.
        delta: ContentBlockDelta,
    },
    /// End of a content block.
    #[serde(rename = "content_block_stop")]
    ContentBlockStop {
        /// Index of the content block.
        index: usize,
    },
    /// Delta update for the message.
    #[serde(rename = "message_delta")]
    MessageDelta {
        /// The delta update.
        delta: MessageDelta,
        /// Updated usage information.
        usage: Usage,
    },
    /// End of the message.
    #[serde(rename = "message_stop")]
    MessageStop,
    /// Ping event (keep-alive).
    #[serde(rename = "ping")]
    Ping,
    /// Error event.
    #[serde(rename = "error")]
    Error {
        /// Error details.
        error: StreamError,
    },
}

/// Initial message in a stream.
#[derive(Debug, Clone, Deserialize)]
pub struct StreamMessage {
    /// Message ID.
    pub id: String,
    /// Model used.
    pub model: String,
    /// Initial usage.
    pub usage: Usage,
}

/// Start of a content block in a stream.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockStart {
    /// Text block start.
    #[serde(rename = "text")]
    Text {
        /// Initial text (usually empty).
        text: String,
    },
    /// Tool use block start.
    #[serde(rename = "tool_use")]
    ToolUse {
        /// Tool use ID.
        id: String,
        /// Tool name.
        name: String,
        /// Initial input (usually empty object).
        input: serde_json::Value,
    },
}

/// Delta update for a content block.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockDelta {
    /// Text delta.
    #[serde(rename = "text_delta")]
    TextDelta {
        /// Text to append.
        text: String,
    },
    /// Input JSON delta (for tool use).
    #[serde(rename = "input_json_delta")]
    InputJsonDelta {
        /// Partial JSON to append.
        partial_json: String,
    },
}

/// Delta update for the message.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageDelta {
    /// Updated stop reason.
    pub stop_reason: Option<StopReason>,
}

/// Error in a stream.
#[derive(Debug, Clone, Deserialize)]
pub struct StreamError {
    /// Error type.
    #[serde(rename = "type")]
    pub error_type: String,
    /// Error message.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_content_text_serialization() {
        let content = MessageContent::Text("Hello".to_string());
        let json = serde_json::to_string(&content).expect("serialize");
        assert_eq!(json, "\"Hello\"");
    }

    #[test]
    fn test_content_block_text_serialization() {
        let block = ContentBlock::Text {
            text: "Hello".to_string(),
        };
        let json = serde_json::to_string(&block).expect("serialize");
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"Hello\""));
    }

    #[test]
    fn test_content_block_tool_use_serialization() {
        let block = ContentBlock::ToolUse {
            id: "tool_123".to_string(),
            name: "get_orders".to_string(),
            input: serde_json::json!({"limit": 10}),
        };
        let json = serde_json::to_string(&block).expect("serialize");
        assert!(json.contains("\"type\":\"tool_use\""));
        assert!(json.contains("\"id\":\"tool_123\""));
        assert!(json.contains("\"name\":\"get_orders\""));
    }

    #[test]
    fn test_stop_reason_deserialization() {
        let json = "\"end_turn\"";
        let reason: StopReason = serde_json::from_str(json).expect("deserialize");
        assert_eq!(reason, StopReason::EndTurn);

        let json = "\"tool_use\"";
        let reason: StopReason = serde_json::from_str(json).expect("deserialize");
        assert_eq!(reason, StopReason::ToolUse);
    }
}

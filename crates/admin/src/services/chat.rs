//! Chat service for orchestrating Claude AI conversations.
//!
//! This service handles the complete flow of:
//! 1. Saving user messages
//! 2. Converting conversation history to Claude format
//! 3. Calling the Claude API
//! 4. Executing tools when requested
//! 5. Saving assistant responses

use askama::Template;
use sqlx::PgPool;
use tracing::{info, instrument, warn};

use naked_pineapple_core::{AdminUserId, ChatRole, ChatSessionId};

use crate::claude::{
    ClaudeClient, ClaudeError, ContentBlock, Message, MessageContent, StopReason, ToolExecutor,
    shopify_tools,
};
use crate::db::{ChatRepository, RepositoryError};
use crate::models::chat::{ChatMessage, ChatSession};
use crate::shopify::AdminClient;

/// System prompt template for the Claude chat assistant.
#[derive(Template)]
#[template(path = "claude/system_prompt.txt")]
struct SystemPromptTemplate;

/// Render the system prompt template.
fn render_system_prompt() -> String {
    // This is a static template with no variables, so it cannot fail.
    // Using a const assertion would be ideal but Askama doesn't support that.
    SystemPromptTemplate
        .render()
        .unwrap_or_else(|_| String::from("You are a helpful assistant."))
}

/// Maximum number of tool use iterations to prevent infinite loops.
const MAX_TOOL_ITERATIONS: usize = 10;

/// Errors that can occur in the chat service.
#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] RepositoryError),

    /// Claude API error.
    #[error("Claude API error: {0}")]
    Claude(#[from] ClaudeError),

    /// Session not found.
    #[error("session not found")]
    SessionNotFound,

    /// Too many tool iterations (possible infinite loop).
    #[error("too many tool iterations")]
    TooManyToolIterations,
}

/// Chat service for orchestrating Claude AI conversations.
pub struct ChatService<'a> {
    pool: &'a PgPool,
    claude: &'a ClaudeClient,
    shopify: &'a AdminClient,
}

impl<'a> ChatService<'a> {
    /// Create a new chat service.
    #[must_use]
    pub const fn new(pool: &'a PgPool, claude: &'a ClaudeClient, shopify: &'a AdminClient) -> Self {
        Self {
            pool,
            claude,
            shopify,
        }
    }

    /// Create a new chat session for an admin user.
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    pub async fn create_session(
        &self,
        admin_user_id: AdminUserId,
    ) -> Result<ChatSession, ChatError> {
        let repo = ChatRepository::new(self.pool);
        Ok(repo.create_session(admin_user_id).await?)
    }

    /// Get a chat session by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    pub async fn get_session(
        &self,
        session_id: ChatSessionId,
    ) -> Result<Option<ChatSession>, ChatError> {
        let repo = ChatRepository::new(self.pool);
        Ok(repo.get_session(session_id).await?)
    }

    /// List chat sessions for an admin user.
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    pub async fn list_sessions(
        &self,
        admin_user_id: AdminUserId,
    ) -> Result<Vec<ChatSession>, ChatError> {
        let repo = ChatRepository::new(self.pool);
        Ok(repo.list_sessions(admin_user_id).await?)
    }

    /// Get all messages in a session.
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    pub async fn get_messages(
        &self,
        session_id: ChatSessionId,
    ) -> Result<Vec<ChatMessage>, ChatError> {
        let repo = ChatRepository::new(self.pool);
        Ok(repo.get_messages(session_id).await?)
    }

    /// Send a message and get a response.
    ///
    /// This handles the complete flow:
    /// 1. Save the user message
    /// 2. Load conversation history
    /// 3. Send to Claude with tools
    /// 4. Execute any tool calls
    /// 5. Loop until Claude responds with text
    /// 6. Save and return all new messages
    ///
    /// # Errors
    ///
    /// Returns an error if any step fails.
    #[instrument(skip(self, user_message), fields(session_id = %session_id))]
    pub async fn send_message(
        &self,
        session_id: ChatSessionId,
        user_message: &str,
    ) -> Result<Vec<ChatMessage>, ChatError> {
        let repo = ChatRepository::new(self.pool);

        // Verify session exists
        if repo.get_session(session_id).await?.is_none() {
            return Err(ChatError::SessionNotFound);
        }

        // Save user message
        let user_content = serde_json::json!({ "text": user_message });
        let user_msg = repo
            .add_message(session_id, ChatRole::User, user_content)
            .await?;

        let mut new_messages = vec![user_msg];

        // Load full conversation history
        let history = repo.get_messages(session_id).await?;

        // Convert to Claude message format
        let mut claude_messages = convert_to_claude_messages(&history);

        // Get available tools and system prompt
        let tools = shopify_tools();
        let system_prompt = render_system_prompt();

        // Tool use loop
        let executor = ToolExecutor::new(self.shopify);
        let mut iterations = 0;

        loop {
            iterations += 1;
            if iterations > MAX_TOOL_ITERATIONS {
                warn!("Too many tool iterations, stopping");
                return Err(ChatError::TooManyToolIterations);
            }

            // Send to Claude
            let response = self
                .claude
                .chat(
                    claude_messages.clone(),
                    Some(system_prompt.clone()),
                    Some(tools.clone()),
                )
                .await?;

            info!(
                stop_reason = ?response.stop_reason,
                content_blocks = response.content.len(),
                "Claude response received"
            );

            // Process response content
            let mut has_tool_use = false;
            let mut tool_results: Vec<ContentBlock> = Vec::new();

            for block in &response.content {
                match block {
                    ContentBlock::Text { text } => {
                        // Save assistant text message
                        let content = serde_json::json!({ "text": text });
                        let msg = repo
                            .add_message(session_id, ChatRole::Assistant, content)
                            .await?;
                        new_messages.push(msg);
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        has_tool_use = true;

                        // Save tool use message
                        let tool_use_content = serde_json::json!({
                            "id": id,
                            "name": name,
                            "input": input
                        });
                        let tool_use_msg = repo
                            .add_message(session_id, ChatRole::ToolUse, tool_use_content)
                            .await?;
                        new_messages.push(tool_use_msg);

                        // Execute the tool
                        let result = executor.execute(name, input).await;

                        let (result_content, is_error) = match result {
                            Ok(r) => (r, false),
                            Err(e) => (format!("Error: {e}"), true),
                        };

                        // Save tool result message
                        let tool_result_content = serde_json::json!({
                            "tool_use_id": id,
                            "content": result_content,
                            "is_error": is_error
                        });
                        let tool_result_msg = repo
                            .add_message(session_id, ChatRole::ToolResult, tool_result_content)
                            .await?;
                        new_messages.push(tool_result_msg);

                        // Build tool result for next Claude request
                        tool_results.push(ContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            content: result_content,
                            is_error: Some(is_error),
                        });
                    }
                    ContentBlock::ToolResult { .. } => {
                        // Should not appear in response
                    }
                }
            }

            // If Claude wants to use tools, add results and continue
            if has_tool_use && response.stop_reason == Some(StopReason::ToolUse) {
                // Add assistant message with tool use to conversation
                claude_messages.push(Message {
                    role: "assistant".to_string(),
                    content: MessageContent::Blocks(response.content.clone()),
                });

                // Add tool results as user message
                claude_messages.push(Message {
                    role: "user".to_string(),
                    content: MessageContent::Blocks(tool_results),
                });

                continue;
            }

            // Done - no more tool use
            break;
        }

        // Update session title from first message if not set
        if new_messages.len() == 1 {
            // This is the first message in a new session
            let title = generate_title(user_message);
            let _ = repo.update_session_title(session_id, &title).await;
        }

        Ok(new_messages)
    }
}

/// State for building Claude messages from database messages.
struct MessageBuilder {
    result: Vec<Message>,
    assistant_blocks: Vec<ContentBlock>,
    tool_results: Vec<ContentBlock>,
}

impl MessageBuilder {
    const fn new() -> Self {
        Self {
            result: Vec::new(),
            assistant_blocks: Vec::new(),
            tool_results: Vec::new(),
        }
    }

    fn flush_assistant_blocks(&mut self) {
        if !self.assistant_blocks.is_empty() {
            self.result.push(Message {
                role: "assistant".to_string(),
                content: MessageContent::Blocks(std::mem::take(&mut self.assistant_blocks)),
            });
        }
    }

    fn flush_tool_results(&mut self) {
        if !self.tool_results.is_empty() {
            self.result.push(Message {
                role: "user".to_string(),
                content: MessageContent::Blocks(std::mem::take(&mut self.tool_results)),
            });
        }
    }

    fn add_user_message(&mut self, msg: &ChatMessage) {
        self.flush_assistant_blocks();
        self.flush_tool_results();

        let text = get_json_str(&msg.content, "text");
        self.result.push(Message {
            role: "user".to_string(),
            content: MessageContent::Text(text),
        });
    }

    fn add_assistant_message(&mut self, msg: &ChatMessage) {
        self.flush_tool_results();

        let text = get_json_str(&msg.content, "text");
        self.assistant_blocks.push(ContentBlock::Text { text });
    }

    fn add_tool_use(&mut self, msg: &ChatMessage) {
        self.flush_tool_results();

        let id = get_json_str(&msg.content, "id");
        let name = get_json_str(&msg.content, "name");
        let input = msg
            .content
            .get("input")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        self.assistant_blocks
            .push(ContentBlock::ToolUse { id, name, input });
    }

    fn add_tool_result(&mut self, msg: &ChatMessage) {
        self.flush_assistant_blocks();

        let tool_use_id = get_json_str(&msg.content, "tool_use_id");
        let content = get_json_str(&msg.content, "content");
        let is_error = msg
            .content
            .get("is_error")
            .and_then(serde_json::Value::as_bool);

        self.tool_results.push(ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        });
    }

    fn finish(mut self) -> Vec<Message> {
        self.flush_assistant_blocks();
        self.flush_tool_results();
        self.result
    }
}

/// Extract a string from JSON content, returning empty string if not found.
fn get_json_str(content: &serde_json::Value, key: &str) -> String {
    content
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Convert database messages to Claude API message format.
fn convert_to_claude_messages(messages: &[ChatMessage]) -> Vec<Message> {
    let mut builder = MessageBuilder::new();

    for msg in messages {
        match msg.role {
            ChatRole::User => builder.add_user_message(msg),
            ChatRole::Assistant => builder.add_assistant_message(msg),
            ChatRole::ToolUse => builder.add_tool_use(msg),
            ChatRole::ToolResult => builder.add_tool_result(msg),
        }
    }

    builder.finish()
}

/// Generate a session title from the first user message.
fn generate_title(message: &str) -> String {
    const MAX_TITLE_LENGTH: usize = 50;

    let trimmed = message.trim();
    if trimmed.len() <= MAX_TITLE_LENGTH {
        trimmed.to_string()
    } else {
        // Find a good break point
        let truncated = &trimmed[..MAX_TITLE_LENGTH];
        truncated.rfind(' ').map_or_else(
            || format!("{truncated}..."),
            |space_idx| format!("{}...", &truncated[..space_idx]),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_title_short() {
        let title = generate_title("Show me recent orders");
        assert_eq!(title, "Show me recent orders");
    }

    #[test]
    fn test_generate_title_long() {
        let message = "This is a very long message that should be truncated because it exceeds the maximum title length";
        let title = generate_title(message);
        assert!(title.len() <= 53); // MAX_TITLE_LENGTH + "..."
        assert!(title.ends_with("..."));
    }

    #[test]
    fn test_generate_title_trims_whitespace() {
        let title = generate_title("  Hello world  ");
        assert_eq!(title, "Hello world");
    }
}

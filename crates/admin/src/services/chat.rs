//! Chat service for orchestrating Claude AI conversations.
//!
//! This service handles the complete flow of:
//! 1. Saving user messages
//! 2. Converting conversation history to Claude format
//! 3. Calling the Claude API (streaming or non-streaming)
//! 4. Executing tools when requested
//! 5. Saving assistant responses
//!
//! ## Streaming Architecture
//!
//! The `send_message_streaming()` method provides true SSE streaming by:
//! - Streaming text tokens as they arrive from Claude
//! - Accumulating tool use blocks as they stream
//! - Executing tools and continuing the conversation loop
//! - Yielding `ChatStreamEvent` items for real-time UI updates

use std::time::Instant;

use askama::Template;
use async_stream::stream;
use chrono::Utc;
use futures::{Stream, StreamExt};
use serde::Serialize;
use sqlx::PgPool;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use naked_pineapple_core::{AdminUserId, ChatMessageId, ChatRole, ChatSessionId};

use crate::claude::{
    ClaudeClient, ClaudeError, ContentBlock, ContentBlockDelta, ContentBlockStart, Message,
    MessageContent, StopReason, StreamEvent, Tool, ToolExecutor, ToolResult, Usage,
    all_shopify_tools,
};
use crate::db::{ChatRepository, RepositoryError};
use crate::models::chat::{ApiInteraction, ChatMessage, ChatSession};
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

// =============================================================================
// Stream Events
// =============================================================================

/// Events emitted during streaming chat responses.
///
/// These events are sent to the client via SSE for real-time UI updates.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ChatStreamEvent {
    /// Text content is being streamed.
    #[serde(rename = "text_delta")]
    TextDelta {
        /// The text chunk.
        text: String,
    },

    /// A tool use request has started streaming.
    #[serde(rename = "tool_use_start")]
    ToolUseStart {
        /// Unique ID for this tool use.
        id: String,
        /// Name of the tool being called.
        name: String,
    },

    /// A tool use request has completed (input fully received).
    #[serde(rename = "tool_use_complete")]
    ToolUseComplete {
        /// Unique ID for this tool use.
        id: String,
        /// Name of the tool.
        name: String,
        /// Complete input parameters.
        input: serde_json::Value,
    },

    /// A tool has been executed and produced a result.
    #[serde(rename = "tool_result")]
    ToolResult {
        /// ID of the tool use this responds to.
        tool_use_id: String,
        /// Result content from the tool.
        content: String,
        /// Whether the tool execution failed.
        is_error: bool,
    },

    /// A write operation is pending Slack confirmation.
    #[serde(rename = "action_pending")]
    ActionPending {
        /// Unique ID for this pending action.
        action_id: Uuid,
        /// Name of the tool awaiting confirmation.
        tool_name: String,
        /// Tool input parameters.
        tool_input: serde_json::Value,
    },

    /// A pending action has been resolved (approved or rejected).
    #[serde(rename = "action_resolved")]
    ActionResolved {
        /// ID of the resolved action.
        action_id: Uuid,
        /// Whether the action was approved.
        approved: bool,
        /// Result if the action was executed, or rejection reason.
        result: Option<String>,
    },

    /// A message has been saved to the database.
    #[serde(rename = "message_saved")]
    MessageSaved {
        /// The saved message ID.
        message_id: ChatMessageId,
        /// The message role.
        role: String,
    },

    /// API interaction metadata (for debug panel).
    #[serde(rename = "api_interaction")]
    ApiInteractionEvent {
        /// Request ID from Claude.
        request_id: Option<String>,
        /// Model used.
        model: String,
        /// Input tokens consumed.
        input_tokens: i32,
        /// Output tokens generated.
        output_tokens: i32,
        /// Request duration in milliseconds.
        duration_ms: i64,
        /// Stop reason.
        stop_reason: Option<String>,
    },

    /// An error occurred during streaming.
    #[serde(rename = "error")]
    Error {
        /// Error message.
        message: String,
    },

    /// The stream has completed.
    #[serde(rename = "done")]
    Done,
}

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
        let tools = all_shopify_tools();
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
                        let (tool_use_msg, tool_result_msg, tool_result_block) =
                            execute_tool_use(&repo, &executor, session_id, id, name, input).await?;
                        new_messages.push(tool_use_msg);
                        new_messages.push(tool_result_msg);
                        tool_results.push(tool_result_block);
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

    /// Send a message and stream the response.
    ///
    /// This provides true SSE streaming where text tokens are yielded as they
    /// arrive from Claude. Tool use blocks are accumulated and executed, with
    /// results streamed back to the client.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The chat session ID
    /// * `user_message` - The user's message text
    ///
    /// # Returns
    ///
    /// A stream of `ChatStreamEvent` items for real-time UI updates.
    #[instrument(skip(self, user_message), fields(session_id = %session_id))]
    pub fn send_message_streaming(
        &self,
        session_id: ChatSessionId,
        user_message: String,
    ) -> impl Stream<Item = ChatStreamEvent> + Send + 'static {
        stream_chat_message(
            self.pool.clone(),
            self.claude.clone(),
            self.shopify.clone(),
            session_id,
            user_message,
        )
    }
}

/// Stream a chat message response with owned values.
///
/// This is a standalone function that takes ownership of the required dependencies,
/// allowing the returned stream to have `'static` lifetime. Use this when
/// you need to return the stream from an async function (e.g., route handlers).
///
/// # Arguments
///
/// * `pool` - Database connection pool (cheap to clone, uses Arc internally)
/// * `claude` - Claude API client (cheap to clone, uses Arc internally)
/// * `shopify` - Shopify Admin API client (cheap to clone, uses Arc internally)
/// * `session_id` - The chat session ID
/// * `user_message` - The user's message text
///
/// # Returns
///
/// A stream of `ChatStreamEvent` items for real-time UI updates.
#[instrument(skip(pool, claude, shopify, user_message), fields(session_id = %session_id))]
pub fn stream_chat_message(
    pool: PgPool,
    claude: ClaudeClient,
    shopify: AdminClient,
    session_id: ChatSessionId,
    user_message: String,
) -> impl Stream<Item = ChatStreamEvent> + Send + 'static {
    streaming_chat_loop(pool, claude, shopify, session_id, user_message)
}

/// State for accumulating a streaming content block.
#[derive(Debug)]
enum StreamingBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input_json: String,
    },
}

/// Accumulated tool use for execution.
struct AccumulatedToolUse {
    id: String,
    name: String,
    input: serde_json::Value,
}

/// State accumulated while processing a streaming Claude response.
struct StreamingState {
    /// Content blocks being built.
    blocks: Vec<StreamingBlock>,
    /// Currently active block index.
    current_block_index: Option<usize>,
    /// Accumulated text content.
    accumulated_text: String,
    /// Stop reason from Claude.
    stop_reason: Option<StopReason>,
    /// Token usage.
    usage: Usage,
    /// Request ID from Claude.
    request_id: Option<String>,
    /// Model used.
    model: String,
}

impl Default for StreamingState {
    fn default() -> Self {
        Self {
            blocks: Vec::new(),
            current_block_index: None,
            accumulated_text: String::new(),
            stop_reason: None,
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
            },
            request_id: None,
            model: String::new(),
        }
    }
}

/// Execute the streaming chat loop.
///
/// This is separated from the method to allow for cleaner async/stream handling.
/// Takes owned values because the stream! macro captures them for async execution.
///
/// # Allow: `too_many_lines`
///
/// This function exceeds the line limit because it is an async generator using the
/// `stream!` macro, which requires `yield` statements throughout. The logic is:
/// 1. Session validation and user message saving
/// 2. Load history and prepare Claude request
/// 3. Stream Claude response (yielding text deltas)
/// 4. Execute tools if requested (yielding tool events)
/// 5. Loop back to step 3 if more tool calls needed
///
/// This flow cannot be cleanly extracted because:
/// - `yield` can only appear inside the generator, not in helper functions
/// - The state (`claude_messages`, `executor`) must persist across loop iterations
/// - Error handling at each step requires yielding error events and returning early
///
/// Refactoring to callbacks or separate streams would make the code harder to follow
/// and maintain, as the sequential nature of the chat loop is essential to understand.
#[allow(clippy::too_many_lines)]
fn streaming_chat_loop(
    pool: PgPool,
    claude: ClaudeClient,
    shopify: AdminClient,
    session_id: ChatSessionId,
    user_message: String,
) -> impl Stream<Item = ChatStreamEvent> + Send {
    stream! {
        let repo = ChatRepository::new(&pool);

        // Verify session exists
        match repo.get_session(session_id).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                yield ChatStreamEvent::Error {
                    message: "Session not found".to_string(),
                };
                yield ChatStreamEvent::Done;
                return;
            }
            Err(e) => {
                yield ChatStreamEvent::Error {
                    message: format!("Database error: {e}"),
                };
                yield ChatStreamEvent::Done;
                return;
            }
        }

        // Save user message
        let user_content = serde_json::json!({ "text": &user_message });
        let user_msg_result = repo
            .add_message(session_id, ChatRole::User, user_content)
            .await;

        match user_msg_result {
            Ok(msg) => {
                yield ChatStreamEvent::MessageSaved {
                    message_id: msg.id,
                    role: "user".to_string(),
                };
            }
            Err(e) => {
                yield ChatStreamEvent::Error {
                    message: format!("Failed to save user message: {e}"),
                };
                yield ChatStreamEvent::Done;
                return;
            }
        }

        // Check if this is the first message for title generation
        let is_first_message = matches!(
            repo.get_messages(session_id).await,
            Ok(msgs) if msgs.len() == 1
        );

        // Load full conversation history
        let history = match repo.get_messages(session_id).await {
            Ok(h) => h,
            Err(e) => {
                yield ChatStreamEvent::Error {
                    message: format!("Failed to load history: {e}"),
                };
                yield ChatStreamEvent::Done;
                return;
            }
        };

        // Convert to Claude message format
        let mut claude_messages = convert_to_claude_messages(&history);

        // Get available tools and system prompt
        let tools = all_shopify_tools();
        let system_prompt = render_system_prompt();

        // Tool use loop
        let executor = ToolExecutor::new(&shopify);
        let mut iterations = 0;

        loop {
            iterations += 1;
            if iterations > MAX_TOOL_ITERATIONS {
                warn!("Too many tool iterations, stopping");
                yield ChatStreamEvent::Error {
                    message: "Request processing exceeded limits".to_string(),
                };
                yield ChatStreamEvent::Done;
                return;
            }

            let start_time = Instant::now();

            // Call Claude with streaming
            let stream_result = claude
                .chat_stream(
                    claude_messages.clone(),
                    Some(system_prompt.clone()),
                    Some(tools.clone()),
                )
                .await;

            let claude_stream = match stream_result {
                Ok(s) => s,
                Err(e) => {
                    yield ChatStreamEvent::Error {
                        message: format!("Claude API error: {e}"),
                    };
                    yield ChatStreamEvent::Done;
                    return;
                }
            };

            // Process the streaming response
            let mut claude_stream = std::pin::pin!(claude_stream);
            let mut state = StreamingState::default();

            while let Some(event_result) = claude_stream.next().await {
                match event_result {
                    Ok(event) => {
                        for stream_event in process_stream_event(event, &mut state) {
                            yield stream_event;
                        }
                    }
                    Err(e) => {
                        yield ChatStreamEvent::Error {
                            message: format!("Stream error: {e}"),
                        };
                    }
                }
            }

            // Convert duration and token counts safely (saturating at max values)
            let duration_ms = i64::try_from(start_time.elapsed().as_millis()).unwrap_or(i64::MAX);
            let input_tokens = i32::try_from(state.usage.input_tokens).unwrap_or(i32::MAX);
            let output_tokens = i32::try_from(state.usage.output_tokens).unwrap_or(i32::MAX);

            // Emit API interaction metadata
            yield ChatStreamEvent::ApiInteractionEvent {
                request_id: state.request_id.clone(),
                model: state.model.clone(),
                input_tokens,
                output_tokens,
                duration_ms,
                stop_reason: state.stop_reason.map(|r| format!("{r:?}").to_lowercase()),
            };

            // Build API interaction for database storage
            let api_interaction = ApiInteraction {
                request_id: state.request_id.take(),
                model: state.model.clone(),
                input_tokens,
                output_tokens,
                duration_ms,
                stop_reason: state.stop_reason.map(|r| format!("{r:?}").to_lowercase()),
                tools_available: Some(tools.iter().map(|t| t.name.clone()).collect()),
                timestamp: Utc::now(),
            };

            // Collect tool uses and save messages
            let mut tool_uses: Vec<AccumulatedToolUse> = Vec::new();
            let mut response_content_blocks: Vec<ContentBlock> = Vec::new();

            for block in &state.blocks {
                match block {
                    StreamingBlock::Text { text } => {
                        if !text.is_empty() {
                            response_content_blocks.push(ContentBlock::Text { text: text.clone() });

                            // Save assistant text message with API interaction
                            let content = serde_json::json!({ "text": text });
                            if let Ok(msg) = repo
                                .add_message_with_interaction(
                                    session_id,
                                    ChatRole::Assistant,
                                    content,
                                    Some(&api_interaction),
                                )
                                .await
                            {
                                yield ChatStreamEvent::MessageSaved {
                                    message_id: msg.id,
                                    role: "assistant".to_string(),
                                };
                            }
                        }
                    }
                    StreamingBlock::ToolUse { id, name, input_json } => {
                        let input: serde_json::Value = serde_json::from_str(input_json.as_str())
                            .unwrap_or(serde_json::Value::Null);

                        response_content_blocks.push(ContentBlock::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        });

                        // Emit tool use complete event
                        yield ChatStreamEvent::ToolUseComplete {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        };

                        tool_uses.push(AccumulatedToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input,
                        });
                    }
                }
            }

            // If there are tool uses and stop_reason is ToolUse, execute them
            if !tool_uses.is_empty() && state.stop_reason == Some(StopReason::ToolUse) {
                // Add assistant message with tool use to conversation
                claude_messages.push(Message {
                    role: "assistant".to_string(),
                    content: MessageContent::Blocks(response_content_blocks),
                });

                let mut tool_results: Vec<ContentBlock> = Vec::new();

                for tool_use in &tool_uses {
                    // Save tool use message
                    let tool_use_content = serde_json::json!({
                        "id": &tool_use.id,
                        "name": &tool_use.name,
                        "input": &tool_use.input
                    });
                    if let Ok(msg) = repo
                        .add_message(session_id, ChatRole::ToolUse, tool_use_content)
                        .await
                    {
                        yield ChatStreamEvent::MessageSaved {
                            message_id: msg.id,
                            role: "tool_use".to_string(),
                        };
                    }

                    // Execute the tool
                    let result = executor.execute(&tool_use.name, &tool_use.input).await;

                    let (result_content, is_error) = match result {
                        Ok(ToolResult::Success(content)) => (content, false),
                        Ok(ToolResult::RequiresConfirmation { tool_name, .. }) => {
                            // Write operations require confirmation via Slack
                            // For now, return a message indicating this
                            (format!("Action '{tool_name}' requires confirmation. This feature is not yet implemented."), false)
                        }
                        Err(e) => (format!("Error: {e}"), true),
                    };

                    // Emit tool result event
                    yield ChatStreamEvent::ToolResult {
                        tool_use_id: tool_use.id.clone(),
                        content: result_content.clone(),
                        is_error,
                    };

                    // Save tool result message
                    let tool_result_content = serde_json::json!({
                        "tool_use_id": &tool_use.id,
                        "content": &result_content,
                        "is_error": is_error
                    });
                    if let Ok(msg) = repo
                        .add_message(session_id, ChatRole::ToolResult, tool_result_content)
                        .await
                    {
                        yield ChatStreamEvent::MessageSaved {
                            message_id: msg.id,
                            role: "tool_result".to_string(),
                        };
                    }

                    // Build tool result for next Claude request
                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: tool_use.id.clone(),
                        content: result_content,
                        is_error: Some(is_error),
                    });
                }

                // Add tool results as user message
                claude_messages.push(Message {
                    role: "user".to_string(),
                    content: MessageContent::Blocks(tool_results),
                });

                // Continue the loop for Claude's next response
                continue;
            }

            // Done - no more tool use
            break;
        }

        // Update session title from first message if not set
        if is_first_message {
            let title = generate_title(&user_message);
            let _ = repo.update_session_title(session_id, &title).await;
        }

        yield ChatStreamEvent::Done;
    }
}

/// Process a single stream event and update streaming state.
fn process_stream_event(event: StreamEvent, state: &mut StreamingState) -> Vec<ChatStreamEvent> {
    let mut events = Vec::new();

    match event {
        StreamEvent::MessageStart { message } => {
            state.request_id = Some(message.id);
            state.model = message.model;
            state.usage = message.usage;
        }
        StreamEvent::ContentBlockStart {
            index,
            content_block,
        } => {
            state.current_block_index = Some(index);

            match content_block {
                ContentBlockStart::Text { text } => {
                    state.blocks.push(StreamingBlock::Text { text });
                }
                ContentBlockStart::ToolUse { id, name, .. } => {
                    events.push(ChatStreamEvent::ToolUseStart {
                        id: id.clone(),
                        name: name.clone(),
                    });
                    state.blocks.push(StreamingBlock::ToolUse {
                        id,
                        name,
                        input_json: String::new(),
                    });
                }
            }
        }
        StreamEvent::ContentBlockDelta { index, delta } => {
            if let Some(block) = state.blocks.get_mut(index) {
                match (block, delta) {
                    (
                        StreamingBlock::Text { text },
                        ContentBlockDelta::TextDelta { text: delta_text },
                    ) => {
                        text.push_str(&delta_text);
                        state.accumulated_text.push_str(&delta_text);
                        events.push(ChatStreamEvent::TextDelta { text: delta_text });
                    }
                    (
                        StreamingBlock::ToolUse { input_json, .. },
                        ContentBlockDelta::InputJsonDelta { partial_json },
                    ) => {
                        input_json.push_str(&partial_json);
                    }
                    _ => {}
                }
            }
        }
        StreamEvent::ContentBlockStop { .. } => {
            state.current_block_index = None;
        }
        StreamEvent::MessageDelta {
            delta,
            usage: new_usage,
        } => {
            state.stop_reason = delta.stop_reason;
            state.usage = new_usage;
        }
        StreamEvent::MessageStop | StreamEvent::Ping => {
            // Message complete or keep-alive ping - no action needed
        }
        StreamEvent::Error { error } => {
            events.push(ChatStreamEvent::Error {
                message: format!("{}: {}", error.error_type, error.message),
            });
        }
    }

    events
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

/// Execute a tool use block and save the messages.
///
/// Returns (`tool_use_message`, `tool_result_message`, `tool_result_block`).
async fn execute_tool_use(
    repo: &ChatRepository<'_>,
    executor: &ToolExecutor<'_>,
    session_id: ChatSessionId,
    id: &str,
    name: &str,
    input: &serde_json::Value,
) -> Result<(ChatMessage, ChatMessage, ContentBlock), ChatError> {
    // Save tool use message
    let tool_use_content = serde_json::json!({
        "id": id,
        "name": name,
        "input": input
    });
    let tool_use_msg = repo
        .add_message(session_id, ChatRole::ToolUse, tool_use_content)
        .await?;

    // Execute the tool
    let result = executor.execute(name, input).await;
    let (result_content, is_error) = convert_tool_result(result);

    // Save tool result message
    let tool_result_content = serde_json::json!({
        "tool_use_id": id,
        "content": result_content,
        "is_error": is_error
    });
    let tool_result_msg = repo
        .add_message(session_id, ChatRole::ToolResult, tool_result_content)
        .await?;

    // Build tool result for next Claude request
    let tool_result_block = ContentBlock::ToolResult {
        tool_use_id: id.to_string(),
        content: result_content,
        is_error: Some(is_error),
    };

    Ok((tool_use_msg, tool_result_msg, tool_result_block))
}

/// Convert a tool execution result to content string and error flag.
fn convert_tool_result(result: Result<ToolResult, ClaudeError>) -> (String, bool) {
    match result {
        Ok(ToolResult::Success(content)) => (content, false),
        Ok(ToolResult::RequiresConfirmation { tool_name, .. }) => {
            // Write operations require confirmation via Slack (not yet implemented)
            let msg = format!("Action '{tool_name}' requires confirmation. Not yet implemented.");
            (msg, false)
        }
        Err(e) => (format!("Error: {e}"), true),
    }
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

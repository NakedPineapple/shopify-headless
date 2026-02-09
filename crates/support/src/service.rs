//! Support chat service.
//!
//! Orchestrates RAG retrieval, Claude API calls, and tool execution for
//! customer support conversations. Used by the storefront chat routes.

use async_stream::stream;
use futures::{Stream, StreamExt};
use naked_pineapple_core::{SupportConversationId, SupportMessageRole};
use naked_pineapple_services::claude::ClaudeClient;
use naked_pineapple_services::claude::{
    ClaudeError, ContentBlock, ContentBlockDelta, ContentBlockStart, Message, MessageContent,
    StopReason, StreamEvent,
};
use naked_pineapple_services::openai::EmbeddingClient;
use sqlx::PgPool;
use tracing::{debug, error, warn};

use crate::db::conversation::ConversationRepository;
use crate::db::message::MessageRepository;
use crate::error::SupportError;
use crate::models::{ChatStreamEvent, CreateMessageParams};
use crate::{rag, tools};

const MAX_MESSAGES_PER_CONVERSATION: i64 = 50;
const MAX_MESSAGE_LENGTH: usize = 2000;
const MAX_TOOL_ITERATIONS: usize = 10;

/// Parameters for constructing a `SupportChatService`.
#[derive(Clone, Copy)]
pub struct SupportChatServiceParams<'a> {
    pub claude: &'a ClaudeClient,
    pub embedding: &'a EmbeddingClient,
    pub pool: &'a PgPool,
    pub system_prompt: &'a str,
    pub is_authenticated: bool,
}

/// Service that handles support chat interactions.
pub struct SupportChatService<'a> {
    claude: &'a ClaudeClient,
    embedding: &'a EmbeddingClient,
    pool: &'a PgPool,
    system_prompt: &'a str,
    is_authenticated: bool,
}

impl<'a> SupportChatService<'a> {
    #[must_use]
    pub const fn new(params: SupportChatServiceParams<'a>) -> Self {
        Self {
            claude: params.claude,
            embedding: params.embedding,
            pool: params.pool,
            system_prompt: params.system_prompt,
            is_authenticated: params.is_authenticated,
        }
    }

    /// Send a message and get a streaming response.
    ///
    /// Validates the message, saves it, performs RAG retrieval, and streams the
    /// Claude response with tool execution.
    ///
    /// # Errors
    ///
    /// Yields `SupportError` if message validation or database operations fail.
    pub fn send_message_streaming(
        &self,
        conversation_id: SupportConversationId,
        user_message: String,
    ) -> impl Stream<Item = Result<ChatStreamEvent, SupportError>> + '_ {
        stream! {
            if let Err(e) = self.validate_and_save(&user_message, conversation_id).await {
                yield Err(e);
                return;
            }

            let system = self.build_system_prompt(&user_message).await;
            let messages = match self.build_message_history(conversation_id).await {
                Ok(m) => m,
                Err(e) => { yield Err(e); return; }
            };

            let inner = self.run_streaming_loop(conversation_id, system, messages);
            let mut inner = std::pin::pin!(inner);
            while let Some(event) = inner.next().await {
                yield event;
            }
        }
    }

    /// Validate message constraints and persist the user message.
    async fn validate_and_save(
        &self,
        user_message: &str,
        conversation_id: SupportConversationId,
    ) -> Result<(), SupportError> {
        if user_message.len() > MAX_MESSAGE_LENGTH {
            return Err(SupportError::MessageTooLong(MAX_MESSAGE_LENGTH));
        }

        let msg_repo = MessageRepository::new(self.pool);
        let count = msg_repo.count_by_conversation(conversation_id).await?;
        if count >= MAX_MESSAGES_PER_CONVERSATION {
            return Err(SupportError::TooManyMessages(MAX_MESSAGES_PER_CONVERSATION));
        }

        msg_repo
            .create(&CreateMessageParams {
                support_conversation_id: conversation_id,
                role: SupportMessageRole::Customer,
                content: serde_json::json!({ "text": user_message }),
                api_interaction: None,
                admin_user_id: None,
            })
            .await?;

        ConversationRepository::new(self.pool)
            .touch_customer_message(conversation_id)
            .await?;

        Ok(())
    }

    /// Build the system prompt, injecting RAG context from the knowledge base.
    async fn build_system_prompt(&self, user_message: &str) -> String {
        let rag_context =
            rag::retrieve_context(self.embedding, self.pool, user_message).await;
        let mut system = self.system_prompt.to_string();
        if !rag_context.is_empty() {
            system.push_str("\n\n## Relevant Knowledge\n\n");
            system.push_str(&rag_context);
        }
        system
    }

    /// Stream Claude responses with tool execution loop.
    fn run_streaming_loop(
        &self,
        conversation_id: SupportConversationId,
        system: String,
        mut messages: Vec<Message>,
    ) -> impl Stream<Item = Result<ChatStreamEvent, SupportError>> + '_ {
        let tool_defs = tools::support_tools(self.is_authenticated);
        stream! {
            for _iteration in 0..MAX_TOOL_ITERATIONS {
                let api_stream = match self.claude.chat_stream(
                    messages.clone(), Some(system.clone()), Some(tool_defs.clone()),
                ).await {
                    Ok(s) => s,
                    Err(e) => {
                        error!(error = %e, "Claude API stream request failed");
                        yield Ok(ChatStreamEvent::Error {
                            message: "I'm sorry, I'm having trouble right now. \
                                Please try again in a moment.".to_string(),
                        });
                        return;
                    }
                };

                let mut collector = StreamCollector::new();
                let mut api_stream = std::pin::pin!(api_stream);
                while let Some(event) = api_stream.next().await {
                    if let Some(chat_event) = collector.process_event(event) {
                        yield Ok(chat_event);
                    }
                    if collector.errored {
                        return;
                    }
                }

                if !collector.needs_tool_continuation() {
                    self.save_assistant_response(conversation_id, &collector.text).await;
                    yield Ok(ChatStreamEvent::MessageComplete);
                    return;
                }

                messages.push(Message {
                    role: "assistant".to_string(),
                    content: MessageContent::Blocks(collector.build_assistant_content()),
                });

                let results = self.execute_tools(conversation_id, &collector).await;
                for event in &results.events {
                    yield Ok(event.clone());
                }
                messages.push(Message {
                    role: "user".to_string(),
                    content: MessageContent::Blocks(results.blocks),
                });
            }

            warn!(conversation_id = %conversation_id, "Hit max tool iterations");
            yield Ok(ChatStreamEvent::Error {
                message: "I'm having trouble completing this request. \
                    Let me connect you with our team.".to_string(),
            });
        }
    }

    /// Persist the assistant's final response text.
    async fn save_assistant_response(
        &self,
        conversation_id: SupportConversationId,
        text: &str,
    ) {
        if text.is_empty() {
            return;
        }
        if let Err(e) = MessageRepository::new(self.pool)
            .create(&CreateMessageParams {
                support_conversation_id: conversation_id,
                role: SupportMessageRole::Assistant,
                content: serde_json::json!({ "text": text }),
                api_interaction: None,
                admin_user_id: None,
            })
            .await
        {
            error!(error = %e, "Failed to save assistant response");
        }
    }

    /// Build message history from the database for a Claude API call.
    async fn build_message_history(
        &self,
        conversation_id: SupportConversationId,
    ) -> Result<Vec<Message>, SupportError> {
        let db_messages = MessageRepository::new(self.pool)
            .list_by_conversation(conversation_id)
            .await?;

        let mut messages = Vec::new();
        for msg in &db_messages {
            let role = match msg.role {
                SupportMessageRole::Customer => "user",
                SupportMessageRole::Assistant | SupportMessageRole::Agent => "assistant",
                SupportMessageRole::System
                | SupportMessageRole::ToolUse
                | SupportMessageRole::ToolResult => continue,
            };

            let text = msg
                .content
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            messages.push(Message {
                role: role.to_string(),
                content: MessageContent::Text(text),
            });
        }

        Ok(messages)
    }

    /// Execute all pending tool calls and collect results.
    async fn execute_tools(
        &self,
        conversation_id: SupportConversationId,
        collector: &StreamCollector,
    ) -> ToolExecutionResults {
        let mut events = Vec::new();
        let mut blocks = Vec::new();
        for (id, name, input) in &collector.tool_uses {
            debug!(tool = %name, "Executing support tool");
            let result = self.execute_tool(name, input, conversation_id).await;
            events.push(ChatStreamEvent::ToolResult {
                tool_use_id: id.clone(),
            });
            blocks.push(ContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: result,
                is_error: None,
            });
        }
        ToolExecutionResults { events, blocks }
    }

    /// Dispatch a single tool call and return the result text.
    async fn execute_tool(
        &self,
        name: &str,
        input: &serde_json::Value,
        conversation_id: SupportConversationId,
    ) -> String {
        match name {
            "lookup_faq" => self.execute_faq_lookup(input).await,
            "lookup_product" => {
                "Product lookup is not yet available. Please describe what \
                    product you're looking for and I'll help with what I know."
                    .to_string()
            }
            "lookup_order_status" => {
                if !self.is_authenticated {
                    return "The customer is not logged in. Please ask them to \
                        log in to their account first so you can look up their order."
                        .to_string();
                }
                "Order lookup is not yet available.".to_string()
            }
            "lookup_subscription" => {
                if !self.is_authenticated {
                    return "The customer is not logged in. Please ask them to \
                        log in to their account first to view subscription details."
                        .to_string();
                }
                "Subscription lookup is not yet available.".to_string()
            }
            "request_human_help" => self.execute_handoff(input, conversation_id).await,
            _ => format!("Unknown tool: {name}"),
        }
    }

    /// Execute the FAQ lookup tool via RAG retrieval.
    async fn execute_faq_lookup(&self, input: &serde_json::Value) -> String {
        let query = input.get("query").and_then(|v| v.as_str()).unwrap_or("");
        if query.is_empty() {
            return "Please provide a search query.".to_string();
        }

        let context = rag::retrieve_context(self.embedding, self.pool, query).await;
        if context.is_empty() {
            "No relevant FAQ content found for that query.".to_string()
        } else {
            context
        }
    }

    /// Execute the human handoff tool — escalate and create a ticket.
    async fn execute_handoff(
        &self,
        input: &serde_json::Value,
        conversation_id: SupportConversationId,
    ) -> String {
        if !self.is_authenticated {
            return "The customer needs to log in or create an account before being connected \
                to a human agent. This ensures our team can follow up with them. Please let the \
                customer know they can log in via the account menu, or create an account with \
                their email address."
                .to_string();
        }

        let reason = input
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("Customer needs human assistance");

        let category = input
            .get("category")
            .and_then(|v| v.as_str())
            .map(String::from);

        let conv_repo = ConversationRepository::new(self.pool);
        if let Err(e) = conv_repo.escalate(conversation_id, reason).await {
            error!(error = %e, "Failed to escalate conversation");
            return "I'm sorry, I had trouble creating a support ticket. Please email us at \
                support@nakedpineapple.co and we'll help you right away."
                .to_string();
        }

        let ticket_repo = crate::db::ticket::TicketRepository::new(self.pool);
        if let Err(e) = ticket_repo
            .create(&crate::models::CreateTicketParams {
                support_conversation_id: conversation_id,
                category,
                priority: "normal".to_string(),
            })
            .await
        {
            error!(error = %e, "Failed to create support ticket");
        }

        "I've created a support ticket and our team will be notified. A human agent will \
            review your conversation and follow up with you soon. Is there anything else I \
            can help with in the meantime?"
            .to_string()
    }
}

/// Collects state from a Claude API streaming response.
///
/// Tracks assistant text, tool uses in progress, and the stop reason.
/// Returns `ChatStreamEvent` items to yield to the client as they arrive.
struct StreamCollector {
    text: String,
    tool_uses: Vec<(String, String, serde_json::Value)>,
    current_tool_id: String,
    current_tool_name: String,
    current_tool_input_json: String,
    stop_reason: Option<StopReason>,
    errored: bool,
}

impl StreamCollector {
    const fn new() -> Self {
        Self {
            text: String::new(),
            tool_uses: Vec::new(),
            current_tool_id: String::new(),
            current_tool_name: String::new(),
            current_tool_input_json: String::new(),
            stop_reason: None,
            errored: false,
        }
    }

    /// Process a single stream event, returning an optional chat event to yield.
    fn process_event(
        &mut self,
        event: Result<StreamEvent, ClaudeError>,
    ) -> Option<ChatStreamEvent> {
        match event {
            Ok(StreamEvent::ContentBlockStart { content_block, .. }) => {
                if let ContentBlockStart::ToolUse { id, name, .. } = content_block {
                    self.current_tool_id.clone_from(&id);
                    self.current_tool_name.clone_from(&name);
                    self.current_tool_input_json.clear();
                    return Some(ChatStreamEvent::ToolUse { id, name });
                }
                None
            }
            Ok(StreamEvent::ContentBlockDelta { delta, .. }) => match delta {
                ContentBlockDelta::TextDelta { text } => {
                    self.text.push_str(&text);
                    Some(ChatStreamEvent::TextDelta { text })
                }
                ContentBlockDelta::InputJsonDelta { partial_json } => {
                    self.current_tool_input_json.push_str(&partial_json);
                    None
                }
            },
            Ok(StreamEvent::ContentBlockStop { .. }) => {
                self.finalize_tool_use();
                None
            }
            Ok(StreamEvent::MessageDelta { delta, .. }) => {
                self.stop_reason = delta.stop_reason;
                None
            }
            Ok(StreamEvent::Error { error }) => {
                error!(
                    error_type = %error.error_type,
                    message = %error.message,
                    "Claude stream error"
                );
                self.errored = true;
                Some(ChatStreamEvent::Error {
                    message: "I'm sorry, something went wrong. Please try again.".to_string(),
                })
            }
            Ok(_) => None,
            Err(e) => {
                error!(error = %e, "Stream processing error");
                self.errored = true;
                Some(ChatStreamEvent::Error {
                    message: "I'm sorry, something went wrong. Please try again.".to_string(),
                })
            }
        }
    }

    /// Finalize a tool use block when its content block stops.
    fn finalize_tool_use(&mut self) {
        if self.current_tool_id.is_empty() {
            return;
        }
        let input = serde_json::from_str(&self.current_tool_input_json)
            .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));
        self.tool_uses.push((
            std::mem::take(&mut self.current_tool_id),
            std::mem::take(&mut self.current_tool_name),
            input,
        ));
        self.current_tool_input_json.clear();
    }

    /// Whether the model stopped to request tool execution.
    fn needs_tool_continuation(&self) -> bool {
        self.stop_reason == Some(StopReason::ToolUse) && !self.tool_uses.is_empty()
    }

    /// Build the assistant message content blocks (text + tool uses).
    fn build_assistant_content(&self) -> Vec<ContentBlock> {
        let mut blocks = Vec::new();
        if !self.text.is_empty() {
            blocks.push(ContentBlock::Text {
                text: self.text.clone(),
            });
        }
        for (id, name, input) in &self.tool_uses {
            blocks.push(ContentBlock::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            });
        }
        blocks
    }
}

/// Results from executing a batch of tool calls.
struct ToolExecutionResults {
    events: Vec<ChatStreamEvent>,
    blocks: Vec<ContentBlock>,
}

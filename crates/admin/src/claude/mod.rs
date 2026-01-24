//! Claude API integration for AI-powered admin chat.
//!
//! # Features
//!
//! - Chat sessions stored in admin `PostgreSQL` database
//! - Message history with tool use support
//! - Integration with Shopify Admin API for data queries
//! - Streaming responses for better UX
//!
//! # Database Schema
//!
//! ```sql
//! CREATE TABLE chat_sessions (
//!     id BIGSERIAL PRIMARY KEY,
//!     admin_user_id BIGINT NOT NULL REFERENCES admin_users(id),
//!     title VARCHAR(255),
//!     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
//!     updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
//! );
//!
//! CREATE TABLE chat_messages (
//!     id BIGSERIAL PRIMARY KEY,
//!     chat_session_id BIGINT NOT NULL REFERENCES chat_sessions(id),
//!     role VARCHAR(50) NOT NULL,  -- user, assistant, tool_use, tool_result
//!     content JSONB NOT NULL,
//!     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
//! );
//!
//! CREATE INDEX idx_chat_messages_session ON chat_messages(chat_session_id);
//! ```
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use serde::{Deserialize, Serialize};
//!
//! pub struct ClaudeClient {
//!     client: reqwest::Client,
//!     api_key: String,
//!     model: String,
//! }
//!
//! impl ClaudeClient {
//!     pub fn new(config: &ClaudeConfig) -> Self {
//!         Self {
//!             client: reqwest::Client::new(),
//!             api_key: config.api_key.clone(),
//!             model: config.model.clone(),
//!         }
//!     }
//!
//!     /// Send a chat message and get a response.
//!     pub async fn chat(
//!         &self,
//!         messages: Vec<Message>,
//!         tools: Option<Vec<Tool>>,
//!     ) -> Result<Response, ClaudeError> {
//!         let request = ChatRequest {
//!             model: self.model.clone(),
//!             max_tokens: 4096,
//!             messages,
//!             tools,
//!         };
//!
//!         let response = self
//!             .client
//!             .post("https://api.anthropic.com/v1/messages")
//!             .header("x-api-key", &self.api_key)
//!             .header("anthropic-version", "2023-06-01")
//!             .header("content-type", "application/json")
//!             .json(&request)
//!             .send()
//!             .await?
//!             .json::<Response>()
//!             .await?;
//!
//!         Ok(response)
//!     }
//!
//!     /// Stream a chat response for real-time display.
//!     pub async fn chat_stream(
//!         &self,
//!         messages: Vec<Message>,
//!         tools: Option<Vec<Tool>>,
//!     ) -> Result<impl Stream<Item = Result<Event, ClaudeError>>, ClaudeError> {
//!         // Use Server-Sent Events streaming
//!         // ...
//!     }
//! }
//!
//! #[derive(Debug, Serialize)]
//! struct ChatRequest {
//!     model: String,
//!     max_tokens: u32,
//!     messages: Vec<Message>,
//!     #[serde(skip_serializing_if = "Option::is_none")]
//!     tools: Option<Vec<Tool>>,
//! }
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct Message {
//!     pub role: String,  // "user" or "assistant"
//!     pub content: MessageContent,
//! }
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! #[serde(untagged)]
//! pub enum MessageContent {
//!     Text(String),
//!     Blocks(Vec<ContentBlock>),
//! }
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! #[serde(tag = "type")]
//! pub enum ContentBlock {
//!     #[serde(rename = "text")]
//!     Text { text: String },
//!     #[serde(rename = "tool_use")]
//!     ToolUse { id: String, name: String, input: serde_json::Value },
//!     #[serde(rename = "tool_result")]
//!     ToolResult { tool_use_id: String, content: String },
//! }
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct Tool {
//!     pub name: String,
//!     pub description: String,
//!     pub input_schema: serde_json::Value,
//! }
//!
//! /// Tools available to Claude for Shopify queries.
//! pub fn shopify_tools() -> Vec<Tool> {
//!     vec![
//!         Tool {
//!             name: "get_orders".to_string(),
//!             description: "Get recent orders from Shopify".to_string(),
//!             input_schema: serde_json::json!({
//!                 "type": "object",
//!                 "properties": {
//!                     "limit": { "type": "integer", "description": "Number of orders to fetch" },
//!                     "status": { "type": "string", "enum": ["open", "closed", "cancelled", "any"] }
//!                 }
//!             }),
//!         },
//!         Tool {
//!             name: "get_products".to_string(),
//!             description: "Get products from Shopify".to_string(),
//!             input_schema: serde_json::json!({
//!                 "type": "object",
//!                 "properties": {
//!                     "limit": { "type": "integer", "description": "Number of products to fetch" },
//!                     "query": { "type": "string", "description": "Search query" }
//!                 }
//!             }),
//!         },
//!         Tool {
//!             name: "get_inventory".to_string(),
//!             description: "Get inventory levels for products".to_string(),
//!             input_schema: serde_json::json!({
//!                 "type": "object",
//!                 "properties": {
//!                     "product_id": { "type": "string", "description": "Product ID to check" }
//!                 }
//!             }),
//!         },
//!     ]
//! }
//!
//! #[derive(Debug, thiserror::Error)]
//! pub enum ClaudeError {
//!     #[error("HTTP error: {0}")]
//!     Http(#[from] reqwest::Error),
//!
//!     #[error("API error: {0}")]
//!     Api(String),
//!
//!     #[error("Rate limited")]
//!     RateLimited,
//! }
//! ```

// TODO: Implement Claude API client

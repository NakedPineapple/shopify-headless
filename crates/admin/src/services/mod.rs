//! Business logic services for admin.
//!
//! # Services
//!
//! - `auth` - Admin authentication (`WebAuthn` only)
//! - `chat` - Claude chat orchestration with tool execution
//! - `email` - Email notifications
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! pub mod auth;
//! pub mod chat;
//! pub mod email;
//!
//! // chat.rs - Orchestrates Claude chat with Shopify tools
//! pub struct ChatService {
//!     pool: PgPool,
//!     claude: ClaudeClient,
//!     shopify: AdminClient,
//! }
//!
//! impl ChatService {
//!     /// Send a message and get a response, executing tools as needed.
//!     pub async fn send_message(
//!         &self,
//!         session_id: i64,
//!         user_message: &str,
//!     ) -> Result<Vec<ChatMessage>, ChatError> {
//!         // 1. Save user message
//!         let user_msg = db::chat::add_message(
//!             &self.pool,
//!             session_id,
//!             "user",
//!             serde_json::json!({ "text": user_message }),
//!         ).await?;
//!
//!         // 2. Load conversation history
//!         let history = db::chat::get_messages(&self.pool, session_id).await?;
//!
//!         // 3. Convert to Claude message format
//!         let messages = history.iter().map(|m| m.into()).collect();
//!
//!         // 4. Send to Claude with Shopify tools
//!         let response = self.claude.chat(
//!             messages,
//!             Some(claude::shopify_tools()),
//!         ).await?;
//!
//!         // 5. Handle tool calls
//!         let mut new_messages = vec![];
//!         for block in response.content {
//!             match block {
//!                 ContentBlock::Text { text } => {
//!                     let msg = db::chat::add_message(
//!                         &self.pool,
//!                         session_id,
//!                         "assistant",
//!                         serde_json::json!({ "text": text }),
//!                     ).await?;
//!                     new_messages.push(msg);
//!                 }
//!                 ContentBlock::ToolUse { id, name, input } => {
//!                     // Execute the tool
//!                     let result = self.execute_tool(&name, &input).await?;
//!
//!                     // Save tool use and result
//!                     let tool_msg = db::chat::add_message(
//!                         &self.pool,
//!                         session_id,
//!                         "tool_use",
//!                         serde_json::json!({ "id": id, "name": name, "input": input }),
//!                     ).await?;
//!                     new_messages.push(tool_msg);
//!
//!                     let result_msg = db::chat::add_message(
//!                         &self.pool,
//!                         session_id,
//!                         "tool_result",
//!                         serde_json::json!({ "tool_use_id": id, "content": result }),
//!                     ).await?;
//!                     new_messages.push(result_msg);
//!
//!                     // Continue conversation with tool result
//!                     // ... recursive call or loop
//!                 }
//!             }
//!         }
//!
//!         Ok(new_messages)
//!     }
//!
//!     async fn execute_tool(&self, name: &str, input: &serde_json::Value) -> Result<String, ChatError> {
//!         match name {
//!             "get_orders" => {
//!                 let limit = input["limit"].as_i64().unwrap_or(10);
//!                 let orders = self.shopify.get_orders(limit, None).await?;
//!                 Ok(serde_json::to_string_pretty(&orders)?)
//!             }
//!             "get_products" => {
//!                 let limit = input["limit"].as_i64().unwrap_or(10);
//!                 let products = self.shopify.get_products(limit, None).await?;
//!                 Ok(serde_json::to_string_pretty(&products)?)
//!             }
//!             "get_inventory" => {
//!                 let product_id = input["product_id"].as_str().ok_or(ChatError::InvalidInput)?;
//!                 let inventory = self.shopify.get_inventory(product_id).await?;
//!                 Ok(serde_json::to_string_pretty(&inventory)?)
//!             }
//!             _ => Err(ChatError::UnknownTool(name.to_string())),
//!         }
//!     }
//! }
//! ```

// TODO: Implement services

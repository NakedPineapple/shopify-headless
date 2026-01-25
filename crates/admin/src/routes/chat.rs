//! Chat route handlers for Claude AI integration.
//!
//! Provides HTTP endpoints for chat sessions and messages.
//! All routes require admin authentication.

use askama::Template;
use axum::response::sse::{Event, KeepAlive};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response, Sse},
    routing::{get, post},
};
use futures::stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

use naked_pineapple_core::ChatSessionId;

use crate::claude::ClaudeClient;
use crate::middleware::RequireAdminAuth;
use crate::models::chat::{ChatMessage, ChatSession};
use crate::services::{ChatError, ChatService};
use crate::state::AppState;

/// Chat page template.
#[derive(Template)]
#[template(path = "chat/index.html")]
struct ChatPageTemplate;

/// Build the chat router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/chat", get(chat_page))
        .route("/chat/sessions", get(list_sessions).post(create_session))
        .route("/chat/sessions/{id}", get(get_session))
        .route("/chat/sessions/{id}/messages", post(send_message))
        .route(
            "/chat/sessions/{id}/messages/stream",
            post(send_message_stream),
        )
}

// =============================================================================
// Request/Response Types
// =============================================================================

/// Request to create a new chat session.
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    // Currently no fields needed, session is created for the authenticated user
}

/// Response for a chat session.
#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub id: i32,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ChatSession> for SessionResponse {
    fn from(session: ChatSession) -> Self {
        Self {
            id: session.id.as_i32(),
            title: session.title,
            created_at: session.created_at.to_rfc3339(),
            updated_at: session.updated_at.to_rfc3339(),
        }
    }
}

/// Response for a chat session with messages.
#[derive(Debug, Serialize)]
pub struct SessionWithMessagesResponse {
    pub session: SessionResponse,
    pub messages: Vec<MessageResponse>,
}

/// Response for a chat message.
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub id: i32,
    pub role: String,
    pub content: serde_json::Value,
    pub created_at: String,
}

impl From<ChatMessage> for MessageResponse {
    fn from(msg: ChatMessage) -> Self {
        Self {
            id: msg.id.as_i32(),
            role: format!("{:?}", msg.role).to_lowercase(),
            content: msg.content,
            created_at: msg.created_at.to_rfc3339(),
        }
    }
}

/// Request to send a message.
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub message: String,
}

/// Response for sending a message (non-streaming).
#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub messages: Vec<MessageResponse>,
}

// =============================================================================
// Error Handling
// =============================================================================

/// Chat API error response.
#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

impl IntoResponse for ChatError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Self::Claude(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            Self::SessionNotFound => (StatusCode::NOT_FOUND, "Session not found".to_string()),
            Self::TooManyToolIterations => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Request processing exceeded limits".to_string(),
            ),
        };

        (status, Json(ErrorResponse { error: message })).into_response()
    }
}

// =============================================================================
// Route Handlers
// =============================================================================

/// Render the chat interface page.
///
/// GET /chat
async fn chat_page(RequireAdminAuth(_admin): RequireAdminAuth) -> impl IntoResponse {
    Html(
        ChatPageTemplate
            .render()
            .unwrap_or_else(|_| String::from("Error rendering template")),
    )
}

/// List chat sessions for the current admin user.
///
/// GET /chat/sessions
async fn list_sessions(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
) -> Result<Json<Vec<SessionResponse>>, ChatError> {
    let claude = ClaudeClient::new(state.config().claude());
    let service = ChatService::new(state.pool(), &claude, state.shopify());

    let sessions = service.list_sessions(admin.id).await?;

    Ok(Json(sessions.into_iter().map(Into::into).collect()))
}

/// Create a new chat session.
///
/// POST /chat/sessions
async fn create_session(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
) -> Result<(StatusCode, Json<SessionResponse>), ChatError> {
    let claude = ClaudeClient::new(state.config().claude());
    let service = ChatService::new(state.pool(), &claude, state.shopify());

    let session = service.create_session(admin.id).await?;

    Ok((StatusCode::CREATED, Json(session.into())))
}

/// Get a chat session with all its messages.
///
/// GET /chat/sessions/:id
async fn get_session(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Result<Json<SessionWithMessagesResponse>, ChatError> {
    let session_id = ChatSessionId::new(id);

    let claude = ClaudeClient::new(state.config().claude());
    let service = ChatService::new(state.pool(), &claude, state.shopify());

    let session = service
        .get_session(session_id)
        .await?
        .ok_or(ChatError::SessionNotFound)?;

    let messages = service.get_messages(session_id).await?;

    Ok(Json(SessionWithMessagesResponse {
        session: session.into(),
        messages: messages.into_iter().map(Into::into).collect(),
    }))
}

/// Send a message and get a response.
///
/// POST /chat/sessions/:id/messages
///
/// Returns all new messages (user message + assistant response + any tool use).
async fn send_message(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<i32>,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, ChatError> {
    let session_id = ChatSessionId::new(id);

    let claude = ClaudeClient::new(state.config().claude());
    let service = ChatService::new(state.pool(), &claude, state.shopify());

    let messages = service.send_message(session_id, &request.message).await?;

    Ok(Json(SendMessageResponse {
        messages: messages.into_iter().map(Into::into).collect(),
    }))
}

/// SSE stream event types for the chat interface.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum StreamEventData {
    #[serde(rename = "assistant_start")]
    AssistantStart,
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "error")]
    Error { message: String },
}

/// Send a message and stream the response via SSE.
///
/// POST /chat/sessions/:id/messages/stream
///
/// Streams events as the assistant responds, including tool use.
async fn send_message_stream(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<i32>,
    Json(request): Json<SendMessageRequest>,
) -> Response {
    let session_id = ChatSessionId::new(id);

    let claude = ClaudeClient::new(state.config().claude());
    let service = ChatService::new(state.pool(), &claude, state.shopify());

    // For now, use the non-streaming approach and emit events for each message.
    // A full streaming implementation would use chat_stream and process events.
    let result = service.send_message(session_id, &request.message).await;

    match result {
        Ok(ref messages) => {
            // Convert messages to SSE events
            let events: Vec<Result<Event, Infallible>> = messages
                .iter()
                .flat_map(message_to_events)
                .chain(std::iter::once(Ok(Event::default().data(
                    serde_json::to_string(&StreamEventData::Done).unwrap_or_default(),
                ))))
                .collect();

            Sse::new(stream::iter(events))
                .keep_alive(KeepAlive::default())
                .into_response()
        }
        Err(e) => {
            let error_event = StreamEventData::Error {
                message: e.to_string(),
            };
            let events: Vec<Result<Event, Infallible>> = vec![Ok(
                Event::default().data(serde_json::to_string(&error_event).unwrap_or_default())
            )];

            Sse::new(stream::iter(events))
                .keep_alive(KeepAlive::default())
                .into_response()
        }
    }
}

/// Convert a chat message to SSE events.
fn message_to_events(msg: &ChatMessage) -> Vec<Result<Event, Infallible>> {
    use naked_pineapple_core::ChatRole;

    match msg.role {
        ChatRole::User => {
            // User messages are shown optimistically, so we don't need to send them
            vec![]
        }
        ChatRole::Assistant => {
            let text = msg
                .content
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            vec![
                Ok(Event::default().data(
                    serde_json::to_string(&StreamEventData::AssistantStart).unwrap_or_default(),
                )),
                Ok(Event::default().data(
                    serde_json::to_string(&StreamEventData::TextDelta { text }).unwrap_or_default(),
                )),
            ]
        }
        ChatRole::ToolUse => {
            let id = msg
                .content
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = msg
                .content
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let input = msg
                .content
                .get("input")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            vec![Ok(Event::default().data(
                serde_json::to_string(&StreamEventData::ToolUse { id, name, input })
                    .unwrap_or_default(),
            ))]
        }
        ChatRole::ToolResult => {
            let tool_use_id = msg
                .content
                .get("tool_use_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = msg
                .content
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let is_error = msg
                .content
                .get("is_error")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);

            vec![Ok(Event::default().data(
                serde_json::to_string(&StreamEventData::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                })
                .unwrap_or_default(),
            ))]
        }
    }
}

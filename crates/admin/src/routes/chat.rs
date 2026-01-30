//! Chat route handlers for Claude AI integration.
//!
//! Provides HTTP endpoints for chat sessions and messages.
//! All routes require admin authentication.

use askama::Template;
use axum::response::sse::{Event, KeepAlive};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response, Sse},
    routing::{delete, get, post},
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

use naked_pineapple_core::{AdminUserId, ChatSessionId};

use crate::claude::ClaudeClient;
use crate::db::ChatRepository;
use crate::filters;
use crate::middleware::RequireAdminAuth;
use crate::models::chat::{ChatMessage, ChatSession};
use crate::routes::dashboard::AdminUserView;
use crate::services::{ChatError, ChatService, ChatStreamEvent, stream_chat_message};
use crate::state::AppState;

/// Chat page template.
#[derive(Template)]
#[template(path = "chat/index.html")]
struct ChatPageTemplate {
    admin_user: AdminUserView,
    current_path: String,
}

/// Build the chat router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/chat", get(chat_page))
        .route("/chat/sessions", get(list_sessions).post(create_session))
        .route(
            "/chat/sessions/{id}",
            get(get_session).delete(delete_session),
        )
        .route("/chat/sessions/{id}/messages", post(send_message))
        .route(
            "/chat/sessions/{id}/messages/stream",
            post(send_message_stream),
        )
        // History routes (server-rendered pages)
        .route("/chat/history", get(history_page))
        .route("/chat/history/{id}", get(history_show))
        .route("/chat/history/{id}/continue", post(history_continue))
        // Debug route (super admin only)
        .route("/chat/sessions/{id}/debug", get(get_session_debug))
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
async fn chat_page(RequireAdminAuth(admin): RequireAdminAuth) -> impl IntoResponse {
    let template = ChatPageTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/chat".to_string(),
    };
    Html(
        template
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

/// Send a message and stream the response via SSE.
///
/// POST /chat/sessions/:id/messages/stream
///
/// Streams events in real-time as Claude generates the response.
/// Text tokens are sent as they arrive, tool use is streamed, and
/// tool results are sent after execution.
async fn send_message_stream(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<i32>,
    Json(request): Json<SendMessageRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let session_id = ChatSessionId::new(id);

    // Clone owned values for the streaming function (all use Arc internally)
    let pool = state.pool().clone();
    let claude = ClaudeClient::new(state.config().claude());
    let shopify = state.shopify().clone();

    // Use true streaming - events are yielded as Claude generates them
    let event_stream = stream_chat_message(pool, claude, shopify, session_id, request.message);

    // Map ChatStreamEvent to SSE Event
    let sse_stream = event_stream.map(|event| {
        let json = serde_json::to_string(&event).unwrap_or_else(|_| {
            r#"{"type":"error","message":"Failed to serialize event"}"#.to_string()
        });
        Ok(Event::default().data(json))
    });

    Sse::new(sse_stream).keep_alive(KeepAlive::default())
}

// =============================================================================
// History Routes (Server-Rendered)
// =============================================================================

/// Query parameters for the history page.
#[derive(Debug, Deserialize)]
pub struct HistoryQueryParams {
    /// Filter by admin user ID (super admin only).
    pub admin_id: Option<i32>,
    /// Page number (1-indexed).
    #[serde(default = "default_page")]
    pub page: i64,
}

const fn default_page() -> i64 {
    1
}

const SESSIONS_PER_PAGE: i64 = 20;

/// Session view for history templates.
#[derive(Debug, Clone)]
pub struct HistorySessionView {
    pub id: i32,
    pub title: String,
    pub admin_user_id: i32,
    pub admin_name: Option<String>,
    pub message_count: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Chat history list page template.
#[derive(Template)]
#[template(path = "chat/history.html")]
struct HistoryPageTemplate {
    admin_user: AdminUserView,
    current_path: String,
    sessions: Vec<HistorySessionView>,
    total_sessions: i64,
    current_page: i64,
    total_pages: i64,
    filter_admin_id: Option<i32>,
}

/// Render the chat history list page.
///
/// GET /chat/history
async fn history_page(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Query(params): Query<HistoryQueryParams>,
) -> impl IntoResponse {
    let is_super_admin = admin.role == crate::models::AdminRole::SuperAdmin;

    // Non-super admins can only see their own sessions
    let filter_admin_id = if is_super_admin {
        params.admin_id
    } else {
        Some(admin.id.as_i32())
    };

    let repo = ChatRepository::new(state.pool());

    // Get total count and paginated sessions
    let admin_filter = filter_admin_id.map(AdminUserId::new);
    let total_sessions = repo.count_sessions(admin_filter).await.unwrap_or(0);
    let total_pages = (total_sessions + SESSIONS_PER_PAGE - 1) / SESSIONS_PER_PAGE;
    let current_page = params.page.clamp(1, total_pages.max(1));
    let offset = (current_page - 1) * SESSIONS_PER_PAGE;

    let sessions = repo
        .list_sessions_paginated(admin_filter, SESSIONS_PER_PAGE, offset)
        .await
        .unwrap_or_default();

    // Convert to view models (we'd need to join with admin_user for names in a real impl)
    let session_views: Vec<HistorySessionView> = sessions
        .into_iter()
        .map(|s| HistorySessionView {
            id: s.id.as_i32(),
            title: s.title.unwrap_or_else(|| "Untitled".to_string()),
            admin_user_id: s.admin_user_id.as_i32(),
            admin_name: None, // Would need join to get this
            message_count: 0, // Would need separate query
            created_at: s.created_at,
            updated_at: s.updated_at,
        })
        .collect();

    let template = HistoryPageTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/chat/history".to_string(),
        sessions: session_views,
        total_sessions,
        current_page,
        total_pages,
        filter_admin_id,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|_| String::from("Error rendering template")),
    )
}

/// Message view for history show template.
#[derive(Debug, Clone)]
pub struct HistoryMessageView {
    pub id: i32,
    pub role: String,
    pub content: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Chat history show page template.
#[derive(Template)]
#[template(path = "chat/history_show.html")]
struct HistoryShowTemplate {
    admin_user: AdminUserView,
    current_path: String,
    session_id: i32,
    session_title: String,
    messages: Vec<HistoryMessageView>,
    can_continue: bool,
}

/// Render a past conversation (read-only view).
///
/// GET /chat/history/:id
async fn history_show(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, Response> {
    let session_id = ChatSessionId::new(id);
    let is_super_admin = admin.role == crate::models::AdminRole::SuperAdmin;

    let repo = ChatRepository::new(state.pool());

    // Get the session
    let session = repo
        .get_session(session_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response())?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found").into_response())?;

    // Check permission: super admin or session owner
    let is_owner = session.admin_user_id == admin.id;
    if !is_super_admin && !is_owner {
        return Err((StatusCode::FORBIDDEN, "Access denied").into_response());
    }

    // Get messages
    let messages = repo
        .get_messages(session_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response())?;

    let message_views: Vec<HistoryMessageView> = messages
        .into_iter()
        .map(|m| HistoryMessageView {
            id: m.id.as_i32(),
            role: format!("{:?}", m.role).to_lowercase(),
            content: m.content,
            created_at: m.created_at,
        })
        .collect();

    let template = HistoryShowTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: format!("/chat/history/{id}"),
        session_id: id,
        session_title: session.title.unwrap_or_else(|| "Untitled".to_string()),
        messages: message_views,
        can_continue: is_owner,
    };

    Ok(Html(template.render().unwrap_or_else(|_| {
        String::from("Error rendering template")
    })))
}

/// Continue a past conversation (redirect to chat page with session selected).
///
/// POST /chat/history/:id/continue
async fn history_continue(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, Response> {
    let session_id = ChatSessionId::new(id);

    let repo = ChatRepository::new(state.pool());

    // Verify session exists and belongs to user
    let is_owner = repo
        .session_belongs_to_admin(session_id, admin.id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response())?;

    if !is_owner {
        return Err((StatusCode::FORBIDDEN, "Access denied").into_response());
    }

    // Redirect to main chat page (JavaScript will select the session)
    Ok(Redirect::to(&format!("/chat?session={id}")))
}

/// Delete a chat session.
///
/// DELETE /chat/sessions/:id
async fn delete_session(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, Response> {
    let session_id = ChatSessionId::new(id);
    let is_super_admin = admin.role == crate::models::AdminRole::SuperAdmin;

    let repo = ChatRepository::new(state.pool());

    // Get the session to check ownership
    let session = repo
        .get_session(session_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response())?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found").into_response())?;

    // Check permission: super admin or session owner
    let is_owner = session.admin_user_id == admin.id;
    if !is_super_admin && !is_owner {
        return Err((StatusCode::FORBIDDEN, "Access denied").into_response());
    }

    // Delete the session
    repo.delete_session(session_id).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to delete session",
        )
            .into_response()
    })?;

    Ok(StatusCode::NO_CONTENT)
}

// =============================================================================
// Debug Panel (Super Admin Only)
// =============================================================================

/// Debug information for a single API interaction.
#[derive(Debug, Serialize)]
pub struct DebugApiCall {
    /// Message ID this interaction belongs to.
    pub message_id: i32,
    /// Request ID from Claude API.
    pub request_id: Option<String>,
    /// Model used.
    pub model: String,
    /// Input tokens.
    pub input_tokens: i32,
    /// Output tokens.
    pub output_tokens: i32,
    /// Duration in milliseconds.
    pub duration_ms: i64,
    /// Stop reason.
    pub stop_reason: Option<String>,
    /// Tools available.
    pub tools_available: Option<Vec<String>>,
    /// Timestamp.
    pub timestamp: String,
}

/// Debug information for a tool use/result pair.
#[derive(Debug, Serialize)]
pub struct DebugToolExecution {
    /// Tool name.
    pub tool_name: String,
    /// Tool input (JSON).
    pub input: serde_json::Value,
    /// Tool result content.
    pub result: Option<String>,
    /// Whether the result was an error.
    pub is_error: bool,
    /// Timestamp.
    pub timestamp: String,
}

/// Pending action for debug display.
#[derive(Debug, Serialize)]
pub struct DebugPendingAction {
    /// Action ID.
    pub id: String,
    /// Tool name.
    pub tool_name: String,
    /// Status (pending, approved, rejected, etc.).
    pub status: String,
    /// Who approved (if approved).
    pub approved_by: Option<String>,
    /// Who rejected (if rejected).
    pub rejected_by: Option<String>,
    /// Created timestamp.
    pub created_at: String,
    /// Resolved timestamp.
    pub resolved_at: Option<String>,
}

/// Complete debug response for a session.
#[derive(Debug, Serialize)]
pub struct SessionDebugResponse {
    /// Session ID.
    pub session_id: i32,
    /// Total input tokens.
    pub total_input_tokens: i32,
    /// Total output tokens.
    pub total_output_tokens: i32,
    /// Total API calls.
    pub total_api_calls: i32,
    /// Total tool calls.
    pub total_tool_calls: i32,
    /// Total duration in milliseconds.
    pub total_duration_ms: i64,
    /// Individual API calls.
    pub api_calls: Vec<DebugApiCall>,
    /// Tool executions.
    pub tool_executions: Vec<DebugToolExecution>,
    /// Pending actions.
    pub pending_actions: Vec<DebugPendingAction>,
}

/// Extracted debug metrics from messages.
struct DebugMetrics {
    total_input_tokens: i32,
    total_output_tokens: i32,
    total_duration_ms: i64,
    api_calls: Vec<DebugApiCall>,
    tool_executions: Vec<DebugToolExecution>,
}

/// Extract debug metrics from chat messages.
fn extract_debug_metrics(messages: &[crate::models::ChatMessage]) -> DebugMetrics {
    let mut metrics = DebugMetrics {
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_duration_ms: 0,
        api_calls: Vec::new(),
        tool_executions: Vec::new(),
    };

    // Track tool uses to pair with results
    let mut pending_tool_uses: std::collections::HashMap<
        String,
        (String, serde_json::Value, String),
    > = std::collections::HashMap::new();

    for msg in messages {
        // Extract API interaction metrics
        if let Some(ref interaction) = msg.api_interaction {
            metrics.total_input_tokens += interaction.input_tokens;
            metrics.total_output_tokens += interaction.output_tokens;
            metrics.total_duration_ms += interaction.duration_ms;

            metrics.api_calls.push(DebugApiCall {
                message_id: msg.id.as_i32(),
                request_id: interaction.request_id.clone(),
                model: interaction.model.clone(),
                input_tokens: interaction.input_tokens,
                output_tokens: interaction.output_tokens,
                duration_ms: interaction.duration_ms,
                stop_reason: interaction.stop_reason.clone(),
                tools_available: interaction.tools_available.clone(),
                timestamp: interaction.timestamp.to_rfc3339(),
            });
        }

        let role_str = format!("{:?}", msg.role).to_lowercase();
        process_tool_use(&role_str, msg, &mut pending_tool_uses);
        process_tool_result(
            &role_str,
            msg,
            &mut pending_tool_uses,
            &mut metrics.tool_executions,
        );
    }

    metrics
}

/// Process tool use messages.
fn process_tool_use(
    role_str: &str,
    msg: &crate::models::ChatMessage,
    pending_tool_uses: &mut std::collections::HashMap<String, (String, serde_json::Value, String)>,
) {
    if role_str != "tooluse" {
        return;
    }
    let Some(id) = msg.content.get("id").and_then(serde_json::Value::as_str) else {
        return;
    };
    let Some(name) = msg.content.get("name").and_then(serde_json::Value::as_str) else {
        return;
    };
    let input = msg
        .content
        .get("input")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    pending_tool_uses.insert(
        id.to_string(),
        (name.to_string(), input, msg.created_at.to_rfc3339()),
    );
}

/// Process tool result messages.
fn process_tool_result(
    role_str: &str,
    msg: &crate::models::ChatMessage,
    pending_tool_uses: &mut std::collections::HashMap<String, (String, serde_json::Value, String)>,
    tool_executions: &mut Vec<DebugToolExecution>,
) {
    if role_str != "toolresult" {
        return;
    }
    let Some(tool_use_id) = msg
        .content
        .get("tool_use_id")
        .and_then(serde_json::Value::as_str)
    else {
        return;
    };
    let Some((name, input, timestamp)) = pending_tool_uses.remove(tool_use_id) else {
        return;
    };
    let result_content = msg
        .content
        .get("content")
        .and_then(serde_json::Value::as_str);
    let is_error = msg
        .content
        .get("is_error")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    tool_executions.push(DebugToolExecution {
        tool_name: name,
        input,
        result: result_content.map(String::from),
        is_error,
        timestamp,
    });
}

/// Get debug information for a session (super admin only).
///
/// GET /chat/sessions/:id/debug
async fn get_session_debug(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Result<Json<SessionDebugResponse>, Response> {
    // Check super admin permission
    if admin.role != crate::models::AdminRole::SuperAdmin {
        return Err((StatusCode::FORBIDDEN, "Super admin access required").into_response());
    }

    let session_id = ChatSessionId::new(id);
    let repo = ChatRepository::new(state.pool());

    // Verify session exists
    let _session = repo
        .get_session(session_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response())?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found").into_response())?;

    // Get all messages and extract debug metrics
    let messages = repo
        .get_messages(session_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response())?;

    let metrics = extract_debug_metrics(&messages);

    // Get pending actions for this session
    let actions_service =
        crate::services::ActionQueueService::new(state.db(), state.slack().cloned());
    let actions = actions_service
        .get_actions_for_session(id)
        .await
        .unwrap_or_default();

    let pending_actions: Vec<DebugPendingAction> = actions
        .into_iter()
        .map(|a| DebugPendingAction {
            id: a.id.to_string(),
            tool_name: a.tool_name,
            status: format!("{:?}", a.status),
            approved_by: a.approved_by,
            rejected_by: a.rejected_by,
            created_at: a.created_at.to_rfc3339(),
            resolved_at: a.resolved_at.map(|dt| dt.to_rfc3339()),
        })
        .collect();

    let api_call_count = i32::try_from(metrics.api_calls.len()).unwrap_or(i32::MAX);
    let tool_call_count = i32::try_from(metrics.tool_executions.len()).unwrap_or(i32::MAX);

    Ok(Json(SessionDebugResponse {
        session_id: id,
        total_input_tokens: metrics.total_input_tokens,
        total_output_tokens: metrics.total_output_tokens,
        total_api_calls: api_call_count,
        total_tool_calls: tool_call_count,
        total_duration_ms: metrics.total_duration_ms,
        api_calls: metrics.api_calls,
        tool_executions: metrics.tool_executions,
        pending_actions,
    }))
}

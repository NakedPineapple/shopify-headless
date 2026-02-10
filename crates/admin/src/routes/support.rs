//! Support inbox route handlers.
//!
//! Provides the admin-facing support inbox for managing customer conversations,
//! tickets, and the knowledge base. Queries run against the storefront database
//! via the restricted `support_pool`.
//!
//! # Routes
//!
//! ```text
//! GET  /support                              -- Inbox page
//! GET  /support/conversations                -- HTMX fragment: filtered conversation list
//! GET  /support/conversations/{id}           -- Conversation detail (HTMX fragment)
//! POST /support/conversations/{id}/reply     -- Send agent reply
//! POST /support/conversations/{id}/assign    -- Assign to admin
//! POST /support/conversations/{id}/resolve   -- Mark resolved
//! GET  /support/tickets                      -- Ticket queue
//! GET  /support/tickets/{id}                 -- Ticket detail
//! POST /support/tickets/{id}/update          -- Update ticket status/priority
//! GET  /support/knowledge                    -- Knowledge base list
//! GET  /support/knowledge/new                -- New entry form
//! POST /support/knowledge                    -- Create entry
//! GET  /support/knowledge/{id}/edit          -- Edit entry form
//! POST /support/knowledge/{id}               -- Update entry
//! POST /support/knowledge/{id}/toggle        -- Toggle active status
//! POST /support/knowledge/{id}/delete        -- Delete entry
//! ```

use askama::Template;

use crate::filters;
use axum::{
    Form, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use naked_pineapple_core::{
    SupportConversationId, SupportConversationStatus, SupportKnowledgeId, SupportMessageRole,
    SupportTicketId,
};
use naked_pineapple_support::{
    db::{
        conversation::ConversationRepository, knowledge::KnowledgeRepository,
        message::MessageRepository, ticket::TicketRepository,
    },
    models::{
        ConversationSummary, CreateKnowledgeParams, CreateMessageParams, SupportConversation,
        SupportKnowledge, SupportMessage, SupportTicket, UpdateKnowledgeParams,
    },
};
use serde::Deserialize;
use tracing::{error, info, warn};

use crate::middleware::auth::RequireAdminAuth;
use crate::models::CurrentAdmin;
use crate::routes::dashboard::AdminUserView;
use crate::state::AppState;

// =============================================================================
// Query / Form Types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct ConversationFilter {
    pub status: Option<String>,
    pub assigned_to: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReplyForm {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct TicketFilter {
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TicketUpdateForm {
    pub status: String,
    pub priority: String,
    pub resolution_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct KnowledgeFilter {
    pub category: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct KnowledgeForm {
    pub title: String,
    pub content: String,
    pub category: String,
}

// =============================================================================
// Templates
// =============================================================================

#[derive(Template)]
#[template(path = "support/inbox.html")]
struct InboxTemplate {
    admin_user: AdminUserView,
    current_path: String,
    conversations: Vec<ConversationSummary>,
    active_filter: String,
    escalated_count: i64,
}

#[derive(Template)]
#[template(path = "support/conversation_list.html")]
struct ConversationListTemplate {
    conversations: Vec<ConversationSummary>,
    active_filter: String,
}

#[derive(Template)]
#[template(path = "support/conversation.html")]
pub struct ConversationTemplate {
    pub conversation: SupportConversation,
    pub messages: Vec<SupportMessage>,
    pub ticket: Option<SupportTicket>,
    pub admin_user: AdminUserView,
    pub detail_target: String,
}

#[derive(Template)]
#[template(path = "support/tickets.html")]
struct TicketsTemplate {
    admin_user: AdminUserView,
    current_path: String,
    tickets: Vec<TicketWithConversation>,
    active_filter: String,
}

pub struct TicketWithConversation {
    pub ticket: SupportTicket,
    pub customer_name: Option<String>,
    pub customer_email: Option<String>,
}

#[derive(Template)]
#[template(path = "support/ticket_detail.html")]
struct TicketDetailTemplate {
    admin_user: AdminUserView,
    current_path: String,
    ticket: SupportTicket,
    conversation: Option<SupportConversation>,
}

#[derive(Template)]
#[template(path = "support/knowledge.html")]
struct KnowledgeTemplate {
    admin_user: AdminUserView,
    current_path: String,
    entries: Vec<SupportKnowledge>,
    active_category: Option<String>,
    categories: Vec<String>,
}

#[derive(Template)]
#[template(path = "support/knowledge_form.html")]
struct KnowledgeFormTemplate {
    admin_user: AdminUserView,
    current_path: String,
    entry: Option<SupportKnowledge>,
    is_edit: bool,
    categories: Vec<String>,
}

// =============================================================================
// Router
// =============================================================================

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/support", get(inbox))
        .route("/support/escalated-count", get(escalated_count_badge))
        .route("/support/conversations", get(conversation_list))
        .route("/support/conversations/{id}", get(conversation_detail))
        .route(
            "/support/conversations/{id}/reply",
            post(conversation_reply),
        )
        .route(
            "/support/conversations/{id}/assign",
            post(conversation_assign),
        )
        .route(
            "/support/conversations/{id}/resolve",
            post(conversation_resolve),
        )
        .route("/support/tickets", get(tickets))
        .route("/support/tickets/{id}", get(ticket_detail))
        .route("/support/tickets/{id}/update", post(ticket_update))
        .route(
            "/support/knowledge",
            get(knowledge_list).post(knowledge_create),
        )
        .route("/support/knowledge/new", get(knowledge_new))
        .route("/support/knowledge/{id}/edit", get(knowledge_edit))
        .route(
            "/support/knowledge/{id}",
            post(knowledge_update).delete(knowledge_delete),
        )
        .route("/support/knowledge/{id}/toggle", post(knowledge_toggle))
}

const KNOWLEDGE_CATEGORIES: &[&str] = &[
    "shipping",
    "returns",
    "ingredients",
    "sizing",
    "subscriptions",
    "payment",
    "general",
];

fn categories() -> Vec<String> {
    KNOWLEDGE_CATEGORIES
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

// =============================================================================
// Inbox
// =============================================================================

async fn inbox(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Query(filter): Query<ConversationFilter>,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return support_disabled_page();
    };

    let active_filter = filter.status.unwrap_or_else(|| "all".to_string());
    let status_filter = parse_status_filter(&active_filter);
    let assigned_to = if filter.assigned_to.as_deref() == Some("me") {
        Some(admin.id.as_i32())
    } else {
        None
    };

    let repo = ConversationRepository::new(pool);
    let conversations = repo
        .list(status_filter, assigned_to, 50, 0)
        .await
        .unwrap_or_default();

    let status_counts = repo.count_by_status().await.unwrap_or_default();
    let escalated_count = status_counts
        .iter()
        .find(|c| c.status == SupportConversationStatus::Escalated)
        .map_or(0, |c| c.count);

    let template = InboxTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/support".to_string(),
        conversations,
        active_filter,
        escalated_count,
    };

    Html(template.render().unwrap_or_default()).into_response()
}

async fn conversation_list(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Query(filter): Query<ConversationFilter>,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let active_filter = filter.status.unwrap_or_else(|| "all".to_string());
    let status_filter = parse_status_filter(&active_filter);
    let assigned_to = if filter.assigned_to.as_deref() == Some("me") {
        Some(admin.id.as_i32())
    } else {
        None
    };

    let repo = ConversationRepository::new(pool);
    let conversations = repo
        .list(status_filter, assigned_to, 50, 0)
        .await
        .unwrap_or_default();

    let template = ConversationListTemplate {
        conversations,
        active_filter,
    };

    Html(template.render().unwrap_or_default()).into_response()
}

async fn conversation_detail(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Response {
    render_conversation(&state, &admin, id, "#conversation-detail").await
}

/// Render a conversation detail fragment with a configurable HTMX target.
pub async fn render_conversation(
    state: &AppState,
    admin: &CurrentAdmin,
    id: i32,
    detail_target: &str,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let conv_id = SupportConversationId::new(id);
    let conv_repo = ConversationRepository::new(pool);
    let msg_repo = MessageRepository::new(pool);
    let ticket_repo = TicketRepository::new(pool);

    let Ok(conversation) = conv_repo.get_by_id(conv_id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let messages = msg_repo
        .list_by_conversation(conv_id)
        .await
        .unwrap_or_default();

    let ticket = ticket_repo
        .get_by_conversation(conv_id)
        .await
        .unwrap_or(None);

    let template = ConversationTemplate {
        conversation,
        messages,
        ticket,
        admin_user: AdminUserView::from(admin),
        detail_target: detail_target.to_string(),
    };

    Html(template.render().unwrap_or_default()).into_response()
}

// =============================================================================
// Conversation Actions
// =============================================================================

async fn conversation_reply(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    headers: HeaderMap,
    Path(id): Path<i32>,
    Form(form): Form<ReplyForm>,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let message = form.message.trim();
    if message.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let conv_id = SupportConversationId::new(id);
    let msg_repo = MessageRepository::new(pool);
    let conv_repo = ConversationRepository::new(pool);

    if let Err(e) = msg_repo
        .create(&CreateMessageParams {
            support_conversation_id: conv_id,
            role: SupportMessageRole::Agent,
            content: serde_json::json!({ "text": message }),
            api_interaction: None,
            admin_user_id: Some(admin.id.as_i32()),
        })
        .await
    {
        error!(error = %e, "Failed to save agent reply");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Update conversation status to waiting (agent replied, waiting for customer)
    let _ = conv_repo
        .update_status(conv_id, SupportConversationStatus::Waiting)
        .await;
    let _ = conv_repo.touch_agent_message(conv_id).await;

    info!(
        conversation_id = id,
        admin_id = admin.id.as_i32(),
        "Agent replied to support conversation"
    );

    // Return the updated conversation detail fragment
    let target = extract_conversation_target(&headers);
    render_conversation(&state, &admin, id, &target).await
}

async fn conversation_assign(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let conv_id = SupportConversationId::new(id);
    let repo = ConversationRepository::new(pool);

    if let Err(e) = repo.assign(conv_id, admin.id.as_i32()).await {
        error!(error = %e, "Failed to assign conversation");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    info!(
        conversation_id = id,
        admin_id = admin.id.as_i32(),
        "Assigned support conversation"
    );

    let target = extract_conversation_target(&headers);
    render_conversation(&state, &admin, id, &target).await
}

async fn conversation_resolve(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    headers: HeaderMap,
    Path(id): Path<i32>,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let conv_id = SupportConversationId::new(id);
    let repo = ConversationRepository::new(pool);

    if let Err(e) = repo.resolve(conv_id).await {
        error!(error = %e, "Failed to resolve conversation");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    info!(
        conversation_id = id,
        admin_id = admin.id.as_i32(),
        "Resolved support conversation"
    );

    let target = extract_conversation_target(&headers);
    render_conversation(&state, &admin, id, &target).await
}

// =============================================================================
// Tickets
// =============================================================================

async fn tickets(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Query(filter): Query<TicketFilter>,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return support_disabled_page();
    };

    let active_filter = filter.status.unwrap_or_else(|| "all".to_string());
    let status_filter = if active_filter == "all" {
        None
    } else {
        Some(active_filter.as_str())
    };

    let ticket_repo = TicketRepository::new(pool);
    let raw_tickets = ticket_repo
        .list(status_filter, 100, 0)
        .await
        .unwrap_or_default();

    // Enrich tickets with customer info from linked conversations
    let conv_repo = ConversationRepository::new(pool);
    let mut tickets_with_conv = Vec::with_capacity(raw_tickets.len());
    for ticket in raw_tickets {
        let (customer_name, customer_email) =
            match conv_repo.get_by_id(ticket.support_conversation_id).await {
                Ok(conv) => (conv.customer_name, conv.customer_email),
                Err(_) => (None, None),
            };
        tickets_with_conv.push(TicketWithConversation {
            ticket,
            customer_name,
            customer_email,
        });
    }

    let template = TicketsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/support/tickets".to_string(),
        tickets: tickets_with_conv,
        active_filter,
    };

    Html(template.render().unwrap_or_default()).into_response()
}

async fn ticket_detail(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return support_disabled_page();
    };

    let ticket_id = SupportTicketId::new(id);
    let ticket_repo = TicketRepository::new(pool);
    let conv_repo = ConversationRepository::new(pool);

    let Ok(ticket) = ticket_repo.get_by_id(ticket_id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let conversation = conv_repo
        .get_by_id(ticket.support_conversation_id)
        .await
        .ok();

    let template = TicketDetailTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/support/tickets".to_string(),
        ticket,
        conversation,
    };

    Html(template.render().unwrap_or_default()).into_response()
}

async fn ticket_update(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
    Form(form): Form<TicketUpdateForm>,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let ticket_id = SupportTicketId::new(id);
    let ticket_repo = TicketRepository::new(pool);

    if form.status == "resolved" {
        if let Err(e) = ticket_repo
            .resolve(ticket_id, form.resolution_notes.as_deref())
            .await
        {
            error!(error = %e, "Failed to resolve ticket");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    } else if let Err(e) = ticket_repo
        .update_status(ticket_id, &form.status, &form.priority)
        .await
    {
        error!(error = %e, "Failed to update ticket");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    info!(
        ticket_id = id,
        admin_id = admin.id.as_i32(),
        status = %form.status,
        "Updated support ticket"
    );

    Redirect::to(&format!("/support/tickets/{id}")).into_response()
}

// =============================================================================
// Knowledge Base
// =============================================================================

async fn knowledge_list(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Query(filter): Query<KnowledgeFilter>,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return support_disabled_page();
    };

    let repo = KnowledgeRepository::new(pool);
    let entries = repo
        .list(filter.category.as_deref(), false)
        .await
        .unwrap_or_default();

    let template = KnowledgeTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/support/knowledge".to_string(),
        entries,
        active_category: filter.category,
        categories: categories(),
    };

    Html(template.render().unwrap_or_default()).into_response()
}

async fn knowledge_new(RequireAdminAuth(admin): RequireAdminAuth) -> Response {
    let template = KnowledgeFormTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/support/knowledge".to_string(),
        entry: None,
        is_edit: false,
        categories: categories(),
    };

    Html(template.render().unwrap_or_default()).into_response()
}

async fn knowledge_create(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Form(form): Form<KnowledgeForm>,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    // Generate embedding
    let embedding = match generate_embedding(&state, &form.title, &form.content).await {
        Ok(e) => e,
        Err(msg) => {
            warn!("{msg}");
            // Use zero vector as fallback when embedding client is unavailable
            vec![0.0; 1536]
        }
    };

    let repo = KnowledgeRepository::new(pool);
    if let Err(e) = repo
        .create(&CreateKnowledgeParams {
            title: form.title,
            content: form.content,
            category: form.category,
            embedding,
            created_by: Some(admin.id.as_i32()),
        })
        .await
    {
        error!(error = %e, "Failed to create knowledge entry");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    info!(admin_id = admin.id.as_i32(), "Created knowledge base entry");
    Redirect::to("/support/knowledge").into_response()
}

async fn knowledge_edit(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return support_disabled_page();
    };

    let repo = KnowledgeRepository::new(pool);
    let Ok(entry) = repo.get_by_id(SupportKnowledgeId::new(id)).await else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let template = KnowledgeFormTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/support/knowledge".to_string(),
        entry: Some(entry),
        is_edit: true,
        categories: categories(),
    };

    Html(template.render().unwrap_or_default()).into_response()
}

async fn knowledge_update(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
    Form(form): Form<KnowledgeForm>,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let embedding = match generate_embedding(&state, &form.title, &form.content).await {
        Ok(e) => e,
        Err(msg) => {
            warn!("{msg}");
            vec![0.0; 1536]
        }
    };

    let repo = KnowledgeRepository::new(pool);
    if let Err(e) = repo
        .update(
            SupportKnowledgeId::new(id),
            &UpdateKnowledgeParams {
                title: form.title,
                content: form.content,
                category: form.category,
                embedding,
            },
        )
        .await
    {
        error!(error = %e, "Failed to update knowledge entry");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    info!(
        knowledge_id = id,
        admin_id = admin.id.as_i32(),
        "Updated knowledge base entry"
    );
    Redirect::to("/support/knowledge").into_response()
}

async fn knowledge_toggle(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let k_id = SupportKnowledgeId::new(id);
    let repo = KnowledgeRepository::new(pool);

    let Ok(entry) = repo.get_by_id(k_id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };

    if let Err(e) = repo.toggle_active(k_id, !entry.is_active).await {
        error!(error = %e, "Failed to toggle knowledge entry");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    Redirect::to("/support/knowledge").into_response()
}

async fn knowledge_delete(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Response {
    let Some(pool) = state.support_pool() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let repo = KnowledgeRepository::new(pool);
    if let Err(e) = repo.delete(SupportKnowledgeId::new(id)).await {
        error!(error = %e, "Failed to delete knowledge entry");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    info!(
        knowledge_id = id,
        admin_id = admin.id.as_i32(),
        "Deleted knowledge base entry"
    );
    Redirect::to("/support/knowledge").into_response()
}

// =============================================================================
// Helpers
// =============================================================================

/// Extract the HTMX target from request headers, falling back to `#conversation-detail`.
fn extract_conversation_target(headers: &HeaderMap) -> String {
    headers
        .get("HX-Target")
        .and_then(|v| v.to_str().ok())
        .map_or_else(|| "#conversation-detail".to_string(), |id| format!("#{id}"))
}

fn parse_status_filter(filter: &str) -> Option<SupportConversationStatus> {
    match filter {
        "escalated" => Some(SupportConversationStatus::Escalated),
        "waiting" => Some(SupportConversationStatus::Waiting),
        "active" => Some(SupportConversationStatus::Active),
        "resolved" => Some(SupportConversationStatus::Resolved),
        _ => None,
    }
}

/// HTMX fragment: returns a badge with the escalated conversation count (or empty).
async fn escalated_count_badge(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
) -> Html<String> {
    let Some(pool) = state.support_pool() else {
        return Html(String::new());
    };

    let repo = ConversationRepository::new(pool);
    let counts = repo.count_by_status().await.unwrap_or_default();
    let escalated = counts
        .iter()
        .find(|c| c.status == SupportConversationStatus::Escalated)
        .map_or(0, |c| c.count);

    if escalated > 0 {
        Html(format!(
            r#"<span class="inline-flex items-center justify-center w-5 h-5 text-xs font-semibold bg-red-500 rounded-full text-white">{escalated}</span>"#
        ))
    } else {
        Html(String::new())
    }
}

fn support_disabled_page() -> Response {
    let html = r#"<!DOCTYPE html><html><head><title>Support - Disabled</title></head>
        <body><div style="padding:2rem;text-align:center;">
        <h1>Support Inbox Unavailable</h1>
        <p>The <code>STOREFRONT_DATABASE_URL</code> environment variable is not configured.</p>
        <p>Set it to the storefront database URL to enable the support inbox.</p>
        <a href="/">Back to Dashboard</a>
        </div></body></html>"#;
    Html(html).into_response()
}

async fn generate_embedding(
    state: &AppState,
    title: &str,
    content: &str,
) -> Result<Vec<f32>, String> {
    let embedding_client = state
        .embedding()
        .ok_or_else(|| "Embedding client not configured".to_string())?;

    let text = format!("{title}\n\n{content}");
    embedding_client
        .embed(&text)
        .await
        .map_err(|e| format!("Embedding generation failed: {e}"))
}

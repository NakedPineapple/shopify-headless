//! Unified ticket queue route handlers.
//!
//! Merges email inbox items and chat support conversations into a single
//! two-panel view. Email data comes from the admin DB, chat data from the
//! storefront DB via the `support_pool`.
//!
//! # Routes
//!
//! ```text
//! GET  /queue                    -- Full page layout
//! GET  /queue/list               -- HTMX fragment: filtered merged item list
//! GET  /queue/detail/email/{id}  -- HTMX fragment: email detail
//! GET  /queue/detail/chat/{id}   -- HTMX fragment: conversation detail
//! GET  /queue/open-count         -- HTMX fragment: sidebar badge
//! ```

use askama::Template;
use axum::{
    Router,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Utc};
use naked_pineapple_core::{SupportConversationId, SupportConversationStatus};
use naked_pineapple_support::db::{
    conversation::ConversationRepository, message::MessageRepository, ticket::TicketRepository,
};
use serde::Deserialize;
use tracing::error;

use crate::db::inbound_email;
use crate::filters;
use crate::middleware::auth::RequireAdminAuth;
use crate::routes::dashboard::AdminUserView;
use crate::state::AppState;

// =============================================================================
// Types
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueSource {
    Email,
    Chat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueStatus {
    Open,
    Waiting,
    Resolved,
}

pub struct QueueItem {
    pub source: QueueSource,
    pub source_id: i32,
    pub queue_status: QueueStatus,
    pub customer_name: Option<String>,
    pub customer_email: Option<String>,
    pub subject: String,
    pub preview: Option<String>,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub needs_attention: bool,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct QueueFilter {
    pub status: Option<String>,
    pub source: Option<String>,
}

// =============================================================================
// Templates
// =============================================================================

#[derive(Template)]
#[template(path = "queue/queue.html")]
struct QueuePageTemplate {
    admin_user: AdminUserView,
    current_path: String,
    active_filter: String,
}

#[derive(Template)]
#[template(path = "queue/queue_list.html")]
struct QueueListTemplate {
    items: Vec<QueueItem>,
}

// =============================================================================
// Router
// =============================================================================

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/queue", get(queue_page))
        .route("/queue/list", get(queue_list))
        .route("/queue/detail/email/{id}", get(queue_email_detail))
        .route("/queue/detail/chat/{id}", get(queue_chat_detail))
        .route("/queue/open-count", get(queue_open_count))
}

// =============================================================================
// Handlers
// =============================================================================

async fn queue_page(
    RequireAdminAuth(admin): RequireAdminAuth,
    Query(filter): Query<QueueFilter>,
) -> Response {
    let active_filter = filter.status.unwrap_or_else(|| "open".to_string());

    let template = QueuePageTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/queue".to_string(),
        active_filter,
    };

    Html(template.render().unwrap_or_default()).into_response()
}

async fn queue_list(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Query(filter): Query<QueueFilter>,
) -> Response {
    let active_filter = filter.status.as_deref().unwrap_or("open");
    let source_filter = filter.source.as_deref();

    let queue_status = match active_filter {
        "all" => None,
        "waiting" => Some(QueueStatus::Waiting),
        "resolved" => Some(QueueStatus::Resolved),
        _ => Some(QueueStatus::Open),
    };

    let (email_items, chat_items) = tokio::join!(
        fetch_email_items(&state, source_filter, queue_status),
        fetch_chat_items(&state, source_filter),
    );

    let mut items: Vec<QueueItem> = email_items.into_iter().chain(chat_items).collect();

    // Chat items are still filtered in-memory (they come from a separate DB).
    if let Some(status) = queue_status {
        items.retain(|item| item.source == QueueSource::Email || item.queue_status == status);
    }

    // Sort by timestamp descending, truncate
    items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    items.truncate(50);

    let template = QueueListTemplate { items };
    Html(template.render().unwrap_or_default()).into_response()
}

async fn queue_email_detail(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Response {
    super::emails::render_detail(&state, id, "#queue-detail").await
}

async fn queue_chat_detail(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Response {
    super::support::render_conversation(&state, &admin, id, "#queue-detail").await
}

async fn queue_open_count(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
) -> Html<String> {
    let email_count = inbound_email::count_open(state.pool()).await.unwrap_or(0);

    let chat_count = if let Some(pool) = state.support_pool() {
        let repo = ConversationRepository::new(pool);
        let counts = repo.count_by_status().await.unwrap_or_default();
        counts
            .iter()
            .filter(|c| {
                matches!(
                    c.status,
                    SupportConversationStatus::Active | SupportConversationStatus::Escalated
                )
            })
            .map(|c| c.count)
            .sum::<i64>()
    } else {
        0
    };

    let total = email_count + chat_count;
    if total > 0 {
        Html(format!(
            r#"<span class="inline-flex items-center justify-center w-5 h-5 rounded-full bg-coral text-white text-xs">{total}</span>"#
        ))
    } else {
        Html(String::new())
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Fetch email items from the admin database and convert to queue items.
///
/// Uses status-specific queries so the DB returns only the rows the caller
/// needs, keeping the list in sync with the badge count from [`count_open`].
async fn fetch_email_items(
    state: &AppState,
    source_filter: Option<&str>,
    queue_status: Option<QueueStatus>,
) -> Vec<QueueItem> {
    if source_filter == Some("chat") {
        return Vec::new();
    }

    let emails = match queue_status {
        Some(QueueStatus::Open | QueueStatus::Waiting) => {
            inbound_email::list_open(state.pool(), 50, 0).await
        }
        Some(QueueStatus::Resolved) => inbound_email::list_resolved(state.pool(), 50, 0).await,
        None => inbound_email::list(state.pool(), None, None, 50, 0).await,
    }
    .unwrap_or_default();

    emails
        .into_iter()
        .map(|e| {
            let queue_status = match e.status.as_str() {
                "responded" | "archived" => QueueStatus::Resolved,
                _ => QueueStatus::Open,
            };
            let needs_attention = e.status == "pending_review";

            QueueItem {
                source: QueueSource::Email,
                source_id: e.id,
                queue_status,
                customer_name: e.from_name,
                customer_email: Some(e.from_address),
                subject: e.subject,
                preview: None,
                category: e.classification,
                priority: None,
                needs_attention,
                timestamp: e.received_at,
            }
        })
        .collect()
}

/// Fetch chat items from the storefront database and convert to queue items.
async fn fetch_chat_items(state: &AppState, source_filter: Option<&str>) -> Vec<QueueItem> {
    if source_filter == Some("email") {
        return Vec::new();
    }

    let Some(pool) = state.support_pool() else {
        return Vec::new();
    };

    let conv_repo = ConversationRepository::new(pool);
    let conversations = conv_repo.list(None, None, 50, 0).await.unwrap_or_default();

    let ticket_repo = TicketRepository::new(pool);
    let mut items = Vec::with_capacity(conversations.len());

    for summary in conversations {
        let conv = &summary.conversation;
        let ticket = ticket_repo
            .get_by_conversation(conv.id)
            .await
            .unwrap_or(None);

        let queue_status = match conv.status {
            SupportConversationStatus::Waiting => QueueStatus::Waiting,
            SupportConversationStatus::Resolved | SupportConversationStatus::Closed => {
                QueueStatus::Resolved
            }
            _ => QueueStatus::Open,
        };

        let needs_attention = conv.status == SupportConversationStatus::Escalated;
        let subject = conv
            .title
            .clone()
            .unwrap_or_else(|| "Chat conversation".to_string());

        items.push(QueueItem {
            source: QueueSource::Chat,
            source_id: conv.id.as_i32(),
            queue_status,
            customer_name: conv.customer_name.clone(),
            customer_email: conv.customer_email.clone(),
            subject,
            preview: summary.last_message_preview.clone(),
            category: ticket.as_ref().and_then(|t| t.category.clone()),
            priority: ticket.map(|t| t.priority),
            needs_attention,
            timestamp: conv.updated_at,
        });
    }

    items
}

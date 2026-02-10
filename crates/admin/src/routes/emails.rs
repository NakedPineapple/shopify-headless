//! Email inbox route handlers.
//!
//! Provides a unified inbox for viewing and acting on inbound emails
//! classified by the automations triage pipeline.
//!
//! # Routes
//!
//! ```text
//! GET  /emails                   -- Inbox page (full layout)
//! GET  /emails/list              -- HTMX fragment: filtered email list
//! GET  /emails/{id}              -- HTMX fragment: email detail
//! POST /emails/{id}/approve      -- Approve + send draft via M365
//! POST /emails/{id}/reject       -- Reject draft
//! POST /emails/{id}/archive      -- Archive email
//! POST /emails/{id}/update-draft -- Save edited draft
//! GET  /emails/pending-count     -- HTMX fragment: sidebar badge count
//! ```

use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tracing::{error, info};

use crate::db::inbound_email;
use crate::filters;
use crate::middleware::auth::RequireAdminAuth;
use crate::routes::dashboard::AdminUserView;
use crate::state::AppState;

// =============================================================================
// Query / Form Types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct InboxFilter {
    pub status: Option<String>,
    pub classification: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DraftForm {
    pub draft: Option<String>,
}

// =============================================================================
// Templates
// =============================================================================

#[derive(Template)]
#[template(path = "emails/inbox.html")]
struct InboxTemplate {
    admin_user: AdminUserView,
    current_path: String,
    active_filter: String,
    pending_review_count: i64,
    status_counts: Vec<inbound_email::StatusCount>,
}

#[derive(Template)]
#[template(path = "emails/email_list.html")]
struct EmailListTemplate {
    emails: Vec<inbound_email::InboundEmailSummary>,
}

#[derive(Template)]
#[template(path = "emails/email_detail.html")]
struct EmailDetailTemplate {
    email: inbound_email::InboundEmailDetail,
    thread: Vec<inbound_email::ThreadMessage>,
    has_m365: bool,
}

// =============================================================================
// Router
// =============================================================================

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/emails", get(inbox))
        .route("/emails/list", get(email_list))
        .route("/emails/pending-count", get(pending_count_badge))
        .route("/emails/{id}", get(email_detail))
        .route("/emails/{id}/approve", post(approve))
        .route("/emails/{id}/reject", post(reject))
        .route("/emails/{id}/archive", post(archive))
        .route("/emails/{id}/update-draft", post(update_draft))
}

// =============================================================================
// Handlers
// =============================================================================

async fn inbox(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Query(filter): Query<InboxFilter>,
) -> Response {
    let pool = state.pool();
    let active_filter = filter
        .status
        .unwrap_or_else(|| "pending_review".to_string());

    let pending_review_count = inbound_email::count_pending_review(pool).await.unwrap_or(0);

    let status_counts = inbound_email::count_by_status(pool)
        .await
        .unwrap_or_default();

    let template = InboxTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/emails".to_string(),
        active_filter,
        pending_review_count,
        status_counts,
    };

    Html(template.render().unwrap_or_default()).into_response()
}

async fn email_list(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Query(filter): Query<InboxFilter>,
) -> Response {
    let pool = state.pool();

    let status_filter = filter.status.as_deref();
    let classification_filter = filter.classification.as_deref();

    let emails = inbound_email::list(pool, status_filter, classification_filter, 50, 0)
        .await
        .unwrap_or_default();

    let template = EmailListTemplate { emails };
    Html(template.render().unwrap_or_default()).into_response()
}

async fn email_detail(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Response {
    let pool = state.pool();

    let email = match inbound_email::get_by_id(pool, id).await {
        Ok(Some(e)) => e,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            error!(error = %e, "failed to fetch email detail");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let thread = inbound_email::get_thread(pool, &email.conversation_id, email.id)
        .await
        .unwrap_or_default();

    let has_m365 = state.m365().is_some();

    let template = EmailDetailTemplate {
        email,
        thread,
        has_m365,
    };

    Html(template.render().unwrap_or_default()).into_response()
}

async fn approve(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
    Form(form): Form<DraftForm>,
) -> Response {
    let pool = state.pool();

    // Save edited draft if provided
    if let Some(ref draft_text) = form.draft
        && let Err(e) = inbound_email::update_draft(pool, id, draft_text).await
    {
        error!(email_id = id, error = %e, "failed to save edited draft");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Mark as approved in DB
    if let Err(e) = inbound_email::approve_draft(pool, id, &admin.name).await {
        error!(email_id = id, error = %e, "failed to approve draft");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Send via M365 if configured
    if let Some(m365) = state.m365() {
        match inbound_email::get_by_id(pool, id).await {
            Ok(Some(email)) => {
                if let Some(ref draft) = email.response_draft {
                    match m365
                        .reply(&email.mailbox, &email.m365_message_id, draft)
                        .await
                    {
                        Ok(()) => {
                            if let Err(e) = inbound_email::mark_response_sent(pool, id).await {
                                error!(email_id = id, error = %e, "failed to mark as sent");
                            }
                            info!(email_id = id, "email reply sent via M365");
                        }
                        Err(e) => {
                            error!(email_id = id, error = %e, "failed to send reply via M365");
                        }
                    }
                }
            }
            Ok(None) => {
                error!(email_id = id, "email not found after approval");
            }
            Err(e) => {
                error!(email_id = id, error = %e, "failed to fetch email for sending");
            }
        }
    }

    // Return updated detail
    render_detail(&state, id).await
}

async fn reject(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Response {
    let pool = state.pool();

    if let Err(e) = inbound_email::reject_draft(pool, id, &admin.name).await {
        error!(email_id = id, error = %e, "failed to reject draft");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    info!(email_id = id, reviewer = %admin.name, "email draft rejected");
    render_detail(&state, id).await
}

async fn archive(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Response {
    let pool = state.pool();

    if let Err(e) = inbound_email::archive_email(pool, id, &admin.name).await {
        error!(email_id = id, error = %e, "failed to archive email");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    info!(email_id = id, reviewer = %admin.name, "email archived");
    render_detail(&state, id).await
}

async fn update_draft(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<i32>,
    Form(form): Form<DraftForm>,
) -> Response {
    let pool = state.pool();

    let Some(draft_text) = form.draft else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    if let Err(e) = inbound_email::update_draft(pool, id, &draft_text).await {
        error!(email_id = id, error = %e, "failed to update draft");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    render_detail(&state, id).await
}

async fn pending_count_badge(
    State(state): State<AppState>,
    RequireAdminAuth(_admin): RequireAdminAuth,
) -> Response {
    let count = inbound_email::count_pending_review(state.pool())
        .await
        .unwrap_or(0);

    if count > 0 {
        Html(format!(
            r#"<span class="inline-flex items-center justify-center w-5 h-5 rounded-full bg-coral text-white text-xs">{count}</span>"#
        ))
        .into_response()
    } else {
        Html(String::new()).into_response()
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Re-render the email detail fragment (used after actions).
async fn render_detail(state: &AppState, id: i32) -> Response {
    let pool = state.pool();

    let email = match inbound_email::get_by_id(pool, id).await {
        Ok(Some(e)) => e,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            error!(error = %e, "failed to re-render email detail");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let thread = inbound_email::get_thread(pool, &email.conversation_id, email.id)
        .await
        .unwrap_or_default();

    let has_m365 = state.m365().is_some();

    let template = EmailDetailTemplate {
        email,
        thread,
        has_m365,
    };

    Html(template.render().unwrap_or_default()).into_response()
}

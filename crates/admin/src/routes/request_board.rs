//! Request board route handlers.
//!
//! Kanban-style board for submitting bug reports and feature requests.

use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use serde::Deserialize;
use tracing::{instrument, warn};

use crate::{
    db::{RepositoryError, RequestBoardRepository},
    error::AppError,
    filters,
    middleware::auth::RequireAdminAuth,
    models::request_board::{
        CreateCommentInput, CreateRequestInput, MoveRequestInput, RequestBoardItemSummary,
        RequestPriority, RequestStatus, RequestType, UpdateRequestInput,
    },
    state::AppState,
};

use super::dashboard::AdminUserView;

// =============================================================================
// Form Inputs
// =============================================================================

/// Form data for creating a request.
#[derive(Debug, Deserialize)]
pub struct CreateRequestForm {
    pub title: String,
    pub description: String,
    pub request_type: String,
    pub priority: String,
}

/// Form data for updating a request.
#[derive(Debug, Deserialize)]
pub struct UpdateRequestForm {
    pub title: String,
    pub description: String,
    pub request_type: String,
    pub priority: String,
}

/// Form data for drag-and-drop status update.
#[derive(Debug, Deserialize)]
pub struct UpdateStatusForm {
    pub status: String,
    pub position: i32,
}

/// Form data for adding a comment.
#[derive(Debug, Deserialize)]
pub struct AddCommentForm {
    pub body: String,
}

// =============================================================================
// View Types
// =============================================================================

/// A card on the Kanban board.
#[derive(Debug, Clone)]
pub struct RequestCardView {
    pub id: i32,
    pub title: String,
    pub request_type: String,
    pub request_type_slug: String,
    pub priority: String,
    pub priority_slug: String,
    pub creator_name: String,
    pub comment_count: i64,
    pub status_slug: String,
}

/// A column on the Kanban board.
#[derive(Debug, Clone)]
pub struct ColumnView {
    pub status: String,
    pub slug: String,
    pub cards: Vec<RequestCardView>,
    pub count: usize,
}

/// Detailed view of a request.
#[derive(Debug, Clone)]
pub struct RequestDetailView {
    pub id: i32,
    pub title: String,
    pub description: String,
    pub request_type: String,
    pub request_type_slug: String,
    pub priority: String,
    pub priority_slug: String,
    pub status: String,
    pub status_slug: String,
    pub creator_name: String,
    pub created_at: String,
    pub updated_at: String,
}

/// View of a comment.
#[derive(Debug, Clone)]
pub struct CommentView {
    pub id: i32,
    pub author_name: String,
    pub body: String,
    pub created_at: String,
}

// =============================================================================
// Conversion Functions
// =============================================================================

/// Convert a summary to a card view.
fn summary_to_card_view(s: &RequestBoardItemSummary) -> RequestCardView {
    RequestCardView {
        id: s.item.id.as_i32(),
        title: s.item.title.clone(),
        request_type: s.item.request_type.label().to_string(),
        request_type_slug: s.item.request_type.slug().to_string(),
        priority: s.item.priority.label().to_string(),
        priority_slug: s.item.priority.slug().to_string(),
        creator_name: s.creator_name.clone(),
        comment_count: s.comment_count,
        status_slug: s.item.status.slug().to_string(),
    }
}

/// Group items into columns for the board view.
fn items_to_columns(items: &[RequestBoardItemSummary]) -> Vec<ColumnView> {
    RequestStatus::all()
        .iter()
        .map(|status| {
            let cards: Vec<RequestCardView> = items
                .iter()
                .filter(|s| s.item.status == *status)
                .map(summary_to_card_view)
                .collect();
            let count = cards.len();
            ColumnView {
                status: status.label().to_string(),
                slug: status.slug().to_string(),
                cards,
                count,
            }
        })
        .collect()
}

/// Convert a summary to a detail view.
fn summary_to_detail_view(s: &RequestBoardItemSummary) -> RequestDetailView {
    RequestDetailView {
        id: s.item.id.as_i32(),
        title: s.item.title.clone(),
        description: s.item.description.clone(),
        request_type: s.item.request_type.label().to_string(),
        request_type_slug: s.item.request_type.slug().to_string(),
        priority: s.item.priority.label().to_string(),
        priority_slug: s.item.priority.slug().to_string(),
        status: s.item.status.label().to_string(),
        status_slug: s.item.status.slug().to_string(),
        creator_name: s.creator_name.clone(),
        created_at: s
            .item
            .created_at
            .format("%b %d, %Y at %l:%M %p")
            .to_string(),
        updated_at: s
            .item
            .updated_at
            .format("%b %d, %Y at %l:%M %p")
            .to_string(),
    }
}

/// Parse a request type from a form string.
fn parse_request_type(s: &str) -> Result<RequestType, AppError> {
    match s {
        "bug_report" => Ok(RequestType::BugReport),
        "feature_request" => Ok(RequestType::FeatureRequest),
        _ => Err(AppError::BadRequest(format!("Invalid request type: {s}"))),
    }
}

/// Parse a priority from a form string.
fn parse_priority(s: &str) -> Result<RequestPriority, AppError> {
    match s {
        "low" => Ok(RequestPriority::Low),
        "medium" => Ok(RequestPriority::Medium),
        "high" => Ok(RequestPriority::High),
        "critical" => Ok(RequestPriority::Critical),
        _ => Err(AppError::BadRequest(format!("Invalid priority: {s}"))),
    }
}

/// Parse a status from a form string.
fn parse_status(s: &str) -> Result<RequestStatus, AppError> {
    match s {
        "new" => Ok(RequestStatus::New),
        "under_review" => Ok(RequestStatus::UnderReview),
        "in_progress" => Ok(RequestStatus::InProgress),
        "done" => Ok(RequestStatus::Done),
        "closed" => Ok(RequestStatus::Closed),
        _ => Err(AppError::BadRequest(format!("Invalid status: {s}"))),
    }
}

// =============================================================================
// Templates
// =============================================================================

/// Kanban board template.
#[derive(Template)]
#[template(path = "request_board/index.html")]
struct BoardTemplate {
    admin_user: AdminUserView,
    current_path: String,
    columns: Vec<ColumnView>,
}

/// Request detail template.
#[derive(Template)]
#[template(path = "request_board/show.html")]
struct ShowTemplate {
    admin_user: AdminUserView,
    current_path: String,
    request: RequestDetailView,
    comments: Vec<CommentView>,
}

/// New request form template.
#[derive(Template)]
#[template(path = "request_board/new.html")]
struct NewTemplate {
    admin_user: AdminUserView,
    current_path: String,
}

/// Edit request form template.
#[derive(Template)]
#[template(path = "request_board/edit.html")]
struct EditTemplate {
    admin_user: AdminUserView,
    current_path: String,
    request: RequestDetailView,
}

/// Comment partial template (for HTMX responses).
#[derive(Template)]
#[template(path = "request_board/_comment.html")]
struct CommentPartialTemplate {
    comment: CommentView,
}

// =============================================================================
// Router
// =============================================================================

/// Build the request board router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/requests", get(index).post(create))
        .route("/requests/new", get(new_request))
        .route("/requests/{id}", get(show).post(update))
        .route("/requests/{id}/edit", get(edit))
        .route("/requests/{id}/status", post(update_status))
        .route("/requests/{id}/comments", post(add_comment))
        .route("/requests/{id}/delete", post(delete))
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /requests — Kanban board view.
#[instrument(skip_all, fields(admin_id = %admin.id.as_i32()))]
async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let repo = RequestBoardRepository::new(state.pool());
    let items = repo.list_all().await?;
    let columns = items_to_columns(&items);

    let template = BoardTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/requests".to_string(),
        columns,
    };

    Ok(Html(template.render().map_err(|e| {
        AppError::Internal(format!("Template error: {e}"))
    })?))
}

/// GET /requests/new — New request form.
#[instrument(skip_all, fields(admin_id = %admin.id.as_i32()))]
async fn new_request(
    RequireAdminAuth(admin): RequireAdminAuth,
) -> Result<impl IntoResponse, AppError> {
    let template = NewTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/requests".to_string(),
    };

    Ok(Html(template.render().map_err(|e| {
        AppError::Internal(format!("Template error: {e}"))
    })?))
}

/// POST /requests — Create a new request.
#[instrument(skip_all, fields(admin_id = %admin.id.as_i32()))]
async fn create(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(form): Form<CreateRequestForm>,
) -> Result<impl IntoResponse, AppError> {
    let request_type = parse_request_type(&form.request_type)?;
    let priority = parse_priority(&form.priority)?;

    let input = CreateRequestInput {
        title: form.title,
        description: form.description,
        request_type,
        priority,
        created_by: admin.id,
    };

    let repo = RequestBoardRepository::new(state.pool());
    repo.create_item(&input).await?;

    Ok(Redirect::to("/requests"))
}

/// GET /requests/{id} — Request detail page with comments.
#[instrument(skip_all, fields(admin_id = %admin.id.as_i32(), item_id = %id))]
async fn show(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, AppError> {
    let repo = RequestBoardRepository::new(state.pool());
    let item_id = naked_pineapple_core::RequestBoardItemId::new(id);

    let summary = repo
        .get_item_with_creator(item_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Request {id} not found")))?;

    let comments_data = repo.list_comments(item_id).await?;
    let comments: Vec<CommentView> = comments_data
        .iter()
        .map(|c| CommentView {
            id: c.comment.id.as_i32(),
            author_name: c.author_name.clone(),
            body: c.comment.body.clone(),
            created_at: c
                .comment
                .created_at
                .format("%b %d, %Y at %l:%M %p")
                .to_string(),
        })
        .collect();

    let template = ShowTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/requests".to_string(),
        request: summary_to_detail_view(&summary),
        comments,
    };

    Ok(Html(template.render().map_err(|e| {
        AppError::Internal(format!("Template error: {e}"))
    })?))
}

/// GET /requests/{id}/edit — Edit request form.
#[instrument(skip_all, fields(admin_id = %admin.id.as_i32(), item_id = %id))]
async fn edit(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, AppError> {
    let repo = RequestBoardRepository::new(state.pool());
    let item_id = naked_pineapple_core::RequestBoardItemId::new(id);

    let summary = repo
        .get_item_with_creator(item_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Request {id} not found")))?;

    let template = EditTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/requests".to_string(),
        request: summary_to_detail_view(&summary),
    };

    Ok(Html(template.render().map_err(|e| {
        AppError::Internal(format!("Template error: {e}"))
    })?))
}

/// POST /requests/{id} — Update a request.
#[instrument(skip_all, fields(admin_id = %admin.id.as_i32(), item_id = %id))]
async fn update(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Form(form): Form<UpdateRequestForm>,
) -> Result<impl IntoResponse, AppError> {
    let request_type = parse_request_type(&form.request_type)?;
    let priority = parse_priority(&form.priority)?;

    let input = UpdateRequestInput {
        title: form.title,
        description: form.description,
        request_type,
        priority,
    };

    let repo = RequestBoardRepository::new(state.pool());
    let item_id = naked_pineapple_core::RequestBoardItemId::new(id);
    repo.update_item(item_id, &input)
        .await
        .map_err(|e| match e {
            RepositoryError::NotFound => AppError::NotFound(format!("Request {id} not found")),
            other => AppError::from(other),
        })?;

    Ok(Redirect::to(&format!("/requests/{id}")))
}

/// POST /requests/{id}/status — Update status via drag-and-drop.
#[instrument(skip_all, fields(admin_id = %admin.id.as_i32(), item_id = %id))]
async fn update_status(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Form(form): Form<UpdateStatusForm>,
) -> Result<impl IntoResponse, AppError> {
    let status = parse_status(&form.status)?;

    let input = MoveRequestInput {
        status,
        position: form.position,
    };

    let repo = RequestBoardRepository::new(state.pool());
    let item_id = naked_pineapple_core::RequestBoardItemId::new(id);
    repo.move_item(item_id, &input).await.map_err(|e| match e {
        RepositoryError::NotFound => AppError::NotFound(format!("Request {id} not found")),
        other => AppError::from(other),
    })?;

    Ok(StatusCode::OK)
}

/// POST /requests/{id}/comments — Add a comment (returns HTMX partial).
#[instrument(skip_all, fields(admin_id = %admin.id.as_i32(), item_id = %id))]
async fn add_comment(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Form(form): Form<AddCommentForm>,
) -> Result<impl IntoResponse, AppError> {
    if form.body.trim().is_empty() {
        return Err(AppError::BadRequest("Comment body is required".to_string()));
    }

    let input = CreateCommentInput {
        item_id: naked_pineapple_core::RequestBoardItemId::new(id),
        admin_user_id: admin.id,
        body: form.body,
    };

    let repo = RequestBoardRepository::new(state.pool());
    let result = repo.create_comment(&input).await?;

    let comment = CommentView {
        id: result.comment.id.as_i32(),
        author_name: result.author_name,
        body: result.comment.body,
        created_at: result
            .comment
            .created_at
            .format("%b %d, %Y at %l:%M %p")
            .to_string(),
    };

    let template = CommentPartialTemplate { comment };
    Ok(Html(template.render().map_err(|e| {
        AppError::Internal(format!("Template error: {e}"))
    })?))
}

/// POST /requests/{id}/delete — Delete a request.
#[instrument(skip_all, fields(admin_id = %admin.id.as_i32(), item_id = %id))]
async fn delete(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, AppError> {
    let repo = RequestBoardRepository::new(state.pool());
    let item_id = naked_pineapple_core::RequestBoardItemId::new(id);

    repo.delete_item(item_id).await.map_err(|e| match e {
        RepositoryError::NotFound => {
            warn!(item_id = %id, "Attempted to delete non-existent request");
            AppError::NotFound(format!("Request {id} not found"))
        }
        other => AppError::from(other),
    })?;

    Ok(Redirect::to("/requests"))
}

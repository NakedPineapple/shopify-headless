//! TikTok Shop returns / refund routes.
//!
//! Provides a view of TikTok Shop return requests cached locally from
//! the TikTok Shop Open API, with approve/reject actions.
//! Only `super_admin` users can access these features.

use askama::Template;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tower_sessions::Session;
use tracing::instrument;

use crate::db::{CachedTikTokReturn, TikTokReturnRepository};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::amazon_settings::ApiResponse;
use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// TikTok returns list page.
#[derive(Template)]
#[template(path = "tiktok/returns/index.html")]
struct ReturnsIndexTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
    returns: Vec<CachedTikTokReturn>,
    total_count: i64,
    page: i64,
    page_size: i64,
    status_filter: Option<String>,
    sync_error: Option<String>,
}

/// TikTok return detail page.
#[derive(Template)]
#[template(path = "tiktok/returns/show.html")]
struct ReturnShowTemplate {
    admin_user: AdminUserView,
    current_path: String,
    return_item: CachedTikTokReturn,
}

// =============================================================================
// Query Parameters
// =============================================================================

#[derive(Debug, Deserialize)]
struct ReturnsQuery {
    page: Option<i64>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RejectRequest {
    reason: String,
}

// =============================================================================
// Router
// =============================================================================

/// Build the TikTok returns router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tiktok/returns", get(returns_index))
        .route("/tiktok/returns/sync", get(returns_sync))
        .route("/tiktok/returns/{id}", get(returns_show))
        .route("/tiktok/returns/{id}/approve", post(returns_approve))
        .route("/tiktok/returns/{id}/reject", post(returns_reject))
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /tiktok/returns -- Returns list.
#[instrument(skip(state, session))]
async fn returns_index(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<ReturnsQuery>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let connected = state.tiktok().is_some();
    if !connected {
        return render(ReturnsIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/tiktok/returns".to_string(),
            connected: false,
            returns: vec![],
            total_count: 0,
            page: 1,
            page_size: 50,
            status_filter: None,
            sync_error: None,
        });
    }

    let page = query.page.unwrap_or(1).max(1);
    let page_size: i64 = 50;
    let offset = (page - 1) * page_size;
    let status_filter = query.status.as_deref();

    let repo = TikTokReturnRepository::new(state.pool());
    let returns = repo.list(page_size, offset, status_filter).await;
    let count = repo.count(status_filter).await;

    match (returns, count) {
        (Ok(returns), Ok(total_count)) => render(ReturnsIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/tiktok/returns".to_string(),
            connected: true,
            returns,
            total_count,
            page,
            page_size,
            status_filter: query.status,
            sync_error: None,
        }),
        (Err(e), _) | (_, Err(e)) => render(ReturnsIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/tiktok/returns".to_string(),
            connected: true,
            returns: vec![],
            total_count: 0,
            page: 1,
            page_size: 50,
            status_filter: None,
            sync_error: Some(format!("Failed to load returns: {e}")),
        }),
    }
}

/// GET /tiktok/returns/sync -- Trigger manual return sync.
#[instrument(skip(state, session))]
async fn returns_sync(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(client) = state.tiktok() else {
        return Redirect::to("/tiktok/returns").into_response();
    };

    match client.get_returns(50, None).await {
        Ok(data) => {
            let repo = TikTokReturnRepository::new(state.pool());
            let mut synced = 0;
            if let Some(returns) = &data.returns {
                for ret in returns {
                    if upsert_return_from_api(&repo, ret).await.is_ok() {
                        synced += 1;
                    }
                }
            }
            tracing::info!(count = synced, "Manual TikTok return sync complete");
        }
        Err(e) => {
            tracing::error!(error = %e, "Manual TikTok return sync failed");
        }
    }

    Redirect::to("/tiktok/returns").into_response()
}

/// GET /tiktok/returns/{id} -- Return detail.
#[instrument(skip(state, session))]
async fn returns_show(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i32>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let repo = TikTokReturnRepository::new(state.pool());
    let return_result = repo.get_by_id(id).await;

    match return_result {
        Ok(Some(return_item)) => render(ReturnShowTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/tiktok/returns".to_string(),
            return_item,
        }),
        _ => Redirect::to("/tiktok/returns").into_response(),
    }
}

/// POST /tiktok/returns/{id}/approve -- Approve a return.
#[instrument(skip(state, session))]
async fn returns_approve(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i32>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(client) = state.tiktok() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("TikTok Shop is not connected")),
        )
            .into_response();
    };

    let repo = TikTokReturnRepository::new(state.pool());
    let Some(return_item) = repo.get_by_id(id).await.ok().flatten() else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Return not found")),
        )
            .into_response();
    };

    match client.approve_return(&return_item.return_id).await {
        Ok(()) => {
            let _ = repo.update_status(&return_item.return_id, "APPROVED").await;
            tracing::info!(return_id = %return_item.return_id, "TikTok return approved");
            Redirect::to(&format!("/tiktok/returns/{id}")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to approve TikTok return");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(format!("Failed to approve return: {e}"))),
            )
                .into_response()
        }
    }
}

/// POST /tiktok/returns/{id}/reject -- Reject a return.
#[instrument(skip(state, session, req))]
async fn returns_reject(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i32>,
    Json(req): Json<RejectRequest>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(client) = state.tiktok() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("TikTok Shop is not connected")),
        )
            .into_response();
    };

    let repo = TikTokReturnRepository::new(state.pool());
    let Some(return_item) = repo.get_by_id(id).await.ok().flatten() else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Return not found")),
        )
            .into_response();
    };

    match client
        .reject_return(&return_item.return_id, &req.reason)
        .await
    {
        Ok(()) => {
            let _ = repo.update_status(&return_item.return_id, "REJECTED").await;
            tracing::info!(return_id = %return_item.return_id, "TikTok return rejected");
            Redirect::to(&format!("/tiktok/returns/{id}")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to reject TikTok return");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(format!("Failed to reject return: {e}"))),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Upsert a single return from the TikTok API response into the DB.
async fn upsert_return_from_api(
    repo: &TikTokReturnRepository<'_>,
    ret: &naked_pineapple_services::tiktok_shop::TikTokReturn,
) -> Result<(), crate::db::RepositoryError> {
    use crate::db::tiktok_returns::UpsertTikTokReturnParams;

    let decision_deadline = ret
        .decision_deadline
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0));

    let params = UpsertTikTokReturnParams {
        return_id: ret.id.as_deref().unwrap_or("unknown"),
        tiktok_order_id: ret.order_id.as_deref(),
        return_status: ret.status.as_deref().unwrap_or("UNKNOWN"),
        return_type: ret.return_type.as_deref(),
        reason: ret.reason.as_deref(),
        buyer_note: ret.buyer_note.as_deref(),
        refund_amount: ret.refund_amount.as_deref(),
        currency: ret.currency.as_deref(),
        decision_deadline,
        return_tracking_number: ret.tracking_number.as_deref(),
    };

    repo.upsert(&params).await?;
    Ok(())
}

/// Get the current admin from the session.
async fn get_admin(session: &Session) -> Option<CurrentAdmin> {
    session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
}

/// Render an Askama template into an HTML response.
fn render(template: impl Template) -> Response {
    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

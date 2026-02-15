//! Faire return / refund routes.
//!
//! Provides a view of Faire return requests cached locally from
//! the Faire Brand API v2, with approve/reject actions.
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

use crate::db::{CachedFaireReturn, FaireReturnRepository};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::amazon_settings::ApiResponse;
use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Faire returns list page.
#[derive(Template)]
#[template(path = "faire/returns/index.html")]
struct FaireReturnsTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
    returns: Vec<CachedFaireReturn>,
    total_count: i64,
    page: i64,
    page_size: i64,
    status_filter: Option<String>,
    sync_error: Option<String>,
}

/// Faire return detail page.
#[derive(Template)]
#[template(path = "faire/returns/show.html")]
struct FaireReturnDetailTemplate {
    admin_user: AdminUserView,
    current_path: String,
    return_item: CachedFaireReturn,
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

/// Build the Faire returns router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/faire/returns", get(returns_index))
        .route("/faire/returns/sync", post(returns_sync))
        .route("/faire/returns/{id}", get(returns_show))
        .route("/faire/returns/{id}/approve", post(returns_approve))
        .route("/faire/returns/{id}/reject", post(returns_reject))
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /faire/returns -- Returns list.
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

    let connected = build_client_from_db(&state).await.is_some();
    if !connected {
        return render(FaireReturnsTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/faire/returns".to_string(),
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

    let repo = FaireReturnRepository::new(state.pool());
    let returns = repo.list(page_size, offset, status_filter).await;
    let count = repo.count(status_filter).await;

    match (returns, count) {
        (Ok(returns), Ok(total_count)) => render(FaireReturnsTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/faire/returns".to_string(),
            connected: true,
            returns,
            total_count,
            page,
            page_size,
            status_filter: query.status,
            sync_error: None,
        }),
        (Err(e), _) | (_, Err(e)) => render(FaireReturnsTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/faire/returns".to_string(),
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

/// POST /faire/returns/sync -- Trigger manual return sync.
#[instrument(skip(state, session))]
async fn returns_sync(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(client) = build_client_from_db(&state).await else {
        return Redirect::to("/faire/returns").into_response();
    };

    match client.list_returns(None, Some(50), None).await {
        Ok(data) => {
            let repo = FaireReturnRepository::new(state.pool());
            let mut synced = 0;
            if let Some(returns) = &data.returns {
                for ret in returns {
                    if upsert_return_from_api(&repo, ret).await.is_ok() {
                        synced += 1;
                    }
                }
            }
            tracing::info!(count = synced, "Manual Faire return sync complete");
        }
        Err(e) => {
            tracing::error!(error = %e, "Manual Faire return sync failed");
        }
    }

    Redirect::to("/faire/returns").into_response()
}

/// GET /faire/returns/{id} -- Return detail.
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

    let repo = FaireReturnRepository::new(state.pool());
    let return_result = repo.get_by_id(id).await;

    match return_result {
        Ok(Some(return_item)) => render(FaireReturnDetailTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/faire/returns".to_string(),
            return_item,
        }),
        _ => Redirect::to("/faire/returns").into_response(),
    }
}

/// POST /faire/returns/{id}/approve -- Approve a return.
#[instrument(skip(state, session))]
async fn returns_approve(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i32>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(client) = build_client_from_db(&state).await else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Faire is not connected")),
        )
            .into_response();
    };

    let repo = FaireReturnRepository::new(state.pool());
    let Some(return_item) = repo.get_by_id(id).await.ok().flatten() else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Return not found")),
        )
            .into_response();
    };

    match client.approve_return(&return_item.faire_return_token).await {
        Ok(_) => {
            let _ = repo
                .update_status(&return_item.faire_return_token, "APPROVED")
                .await;
            tracing::info!(faire_return_token = %return_item.faire_return_token, "Faire return approved");
            Redirect::to(&format!("/faire/returns/{id}")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to approve Faire return");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(format!("Failed to approve return: {e}"))),
            )
                .into_response()
        }
    }
}

/// POST /faire/returns/{id}/reject -- Reject a return.
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

    let Some(client) = build_client_from_db(&state).await else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Faire is not connected")),
        )
            .into_response();
    };

    let repo = FaireReturnRepository::new(state.pool());
    let Some(return_item) = repo.get_by_id(id).await.ok().flatten() else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Return not found")),
        )
            .into_response();
    };

    match client
        .reject_return(&return_item.faire_return_token, &req.reason)
        .await
    {
        Ok(_) => {
            let _ = repo
                .update_status(&return_item.faire_return_token, "REJECTED")
                .await;
            tracing::info!(faire_return_token = %return_item.faire_return_token, "Faire return rejected");
            Redirect::to(&format!("/faire/returns/{id}")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to reject Faire return");
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

/// Convert cents to a dollar string (e.g., 1234 -> "12.34").
fn cents_to_dollars(cents: i64) -> String {
    #[allow(
        clippy::cast_precision_loss,
        reason = "monetary cent values will never approach 2^52"
    )]
    let dollars = cents as f64 / 100.0;
    format!("{dollars:.2}")
}

/// Upsert a single return from the Faire API response into the DB.
async fn upsert_return_from_api(
    repo: &FaireReturnRepository<'_>,
    ret: &naked_pineapple_services::faire::FaireReturn,
) -> Result<(), crate::db::RepositoryError> {
    use crate::db::faire_returns::UpsertFaireReturnParams;

    let raw_json = serde_json::to_value(ret).ok();
    let refund_amount_str = ret.refund_total_cents.map(cents_to_dollars);

    let params = UpsertFaireReturnParams {
        faire_return_token: ret.token.as_deref().unwrap_or("unknown"),
        faire_order_token: ret.order_token.as_deref(),
        return_status: ret.state.as_deref().unwrap_or("UNKNOWN"),
        return_reason: ret.reason.as_deref(),
        retailer_note: ret.retailer_note.as_deref(),
        refund_amount: refund_amount_str.as_deref(),
        currency: None,
        decision_deadline: None,
        raw_json: raw_json.as_ref(),
    };

    repo.upsert(&params).await?;
    Ok(())
}

/// Build a `FaireClient` from stored DB credentials.
async fn build_client_from_db(
    state: &AppState,
) -> Option<naked_pineapple_services::faire::FaireClient> {
    use naked_pineapple_services::faire::FaireCredentials as ApiCredentials;

    let repo = crate::db::FaireCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten()?;

    Some(naked_pineapple_services::faire::FaireClient::new(
        ApiCredentials {
            brand_id: creds.brand_id,
            api_token: creds.api_token,
        },
    ))
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

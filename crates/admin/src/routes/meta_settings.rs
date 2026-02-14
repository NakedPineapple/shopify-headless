//! Meta Commerce settings routes.
//!
//! These routes handle the configuration of the Meta Commerce API
//! integration (Facebook Shop + Instagram Shopping). Only `super_admin`
//! users can manage Meta Commerce settings.

use askama::Template;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tower_sessions::Session;
use tracing::instrument;

use crate::db::{MetaCommerceCredentialsRepository, SaveMetaCommerceParams};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::amazon_settings::ApiResponse;
use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Meta Commerce settings page template.
#[derive(Template)]
#[template(path = "settings/meta.html")]
pub struct MetaSettingsTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub connected: bool,
    pub commerce_account_id: Option<String>,
    pub catalog_id: Option<String>,
    pub last_used_at: Option<String>,
    pub success_message: Option<String>,
    pub error_message: Option<String>,
}

/// Build the Meta Commerce settings router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/settings/meta", get(settings_page))
        .route("/settings/meta/connect", post(connect))
        .route("/settings/meta/disconnect", post(disconnect))
        .route("/settings/meta/test", post(test_connection))
}

// =============================================================================
// Query Parameters
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct SettingsQueryParams {
    pub success: Option<String>,
    pub error: Option<String>,
}

// =============================================================================
// API Types
// =============================================================================

/// Request to connect Meta Commerce credentials.
#[derive(Deserialize)]
pub struct ConnectRequest {
    pub app_id: String,
    pub app_secret: String,
    pub page_access_token: String,
    pub page_id: String,
    pub commerce_account_id: String,
    pub catalog_id: String,
}

impl std::fmt::Debug for ConnectRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectRequest")
            .field("app_id", &self.app_id)
            .field("app_secret", &"[REDACTED]")
            .field("page_access_token", &"[REDACTED]")
            .field("page_id", &self.page_id)
            .field("commerce_account_id", &self.commerce_account_id)
            .field("catalog_id", &self.catalog_id)
            .finish()
    }
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /settings/meta - Meta Commerce settings page.
#[instrument(skip(state, session))]
async fn settings_page(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<SettingsQueryParams>,
) -> Response {
    tracing::debug!("Loading Meta Commerce settings page");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to access Meta Commerce settings");
        return response;
    }

    let Some(admin) = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
    else {
        tracing::warn!("No admin in session for Meta Commerce settings page");
        return Redirect::to("/auth/login").into_response();
    };

    let repo = MetaCommerceCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten();

    let (connected, commerce_account_id, catalog_id, last_used_at) = if let Some(creds) = creds {
        let last_used = creds
            .last_used_at
            .map(|dt| dt.format("%b %d, %Y %H:%M UTC").to_string());

        (
            true,
            Some(creds.commerce_account_id),
            Some(creds.catalog_id),
            last_used,
        )
    } else {
        (false, None, None, None)
    };

    let success_message = params.success.as_deref().map(|s| match s {
        "connected" => "Successfully connected to Meta Commerce!".to_string(),
        "disconnected" => "Successfully disconnected from Meta Commerce.".to_string(),
        "test_passed" => "Connection test successful!".to_string(),
        _ => format!("Success: {s}"),
    });

    let error_message = params.error.as_deref().map(|e| match e {
        "auth_failed" => "Authentication failed. Please check your credentials.".to_string(),
        "disconnect_failed" => "Failed to disconnect from Meta Commerce.".to_string(),
        "test_failed" => "Connection test failed.".to_string(),
        "not_connected" => "Meta Commerce is not connected.".to_string(),
        _ => format!("Error: {e}"),
    });

    let template = MetaSettingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/settings/meta".to_string(),
        connected,
        commerce_account_id,
        catalog_id,
        last_used_at,
        success_message,
        error_message,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

/// POST /settings/meta/connect - Connect Meta Commerce credentials.
#[instrument(skip(state, session, req))]
async fn connect(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<ConnectRequest>,
) -> Response {
    tracing::debug!("Attempting to connect Meta Commerce");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to connect Meta Commerce");
        return response;
    }

    let admin_id = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
        .map(|a| a.id.as_i32());

    if req.commerce_account_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Commerce Account ID is required")),
        )
            .into_response();
    }

    // Save credentials to database
    let repo = MetaCommerceCredentialsRepository::new(state.pool());
    let params = SaveMetaCommerceParams {
        account_name: "default",
        app_id: req.app_id.trim(),
        app_secret: req.app_secret.trim(),
        page_access_token: req.page_access_token.trim(),
        page_id: req.page_id.trim(),
        commerce_account_id: req.commerce_account_id.trim(),
        catalog_id: req.catalog_id.trim(),
        connected_by: admin_id,
    };

    if let Err(e) = repo.save(&params).await {
        tracing::error!(error = %e, "Failed to save Meta Commerce credentials");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error("Failed to save credentials")),
        )
            .into_response();
    }

    // Test the connection with the new credentials
    let client = build_client_from_db(&state).await;
    match client {
        Some(c) => match c.test_connection().await {
            Ok(account_info) => {
                let account_name = account_info.name.as_deref().unwrap_or("Unknown");
                tracing::info!(
                    commerce_account_id = %req.commerce_account_id,
                    account_name = %account_name,
                    "Successfully connected to Meta Commerce"
                );
                (
                    StatusCode::OK,
                    Json(ApiResponse::success(format!(
                        "Connected to Meta Commerce ({account_name})"
                    ))),
                )
                    .into_response()
            }
            Err(e) => {
                tracing::warn!(error = %e, "Meta Commerce connection test failed after save");
                // Credentials saved but test failed -- keep them so user can fix
                (
                    StatusCode::OK,
                    Json(ApiResponse::success(
                        "Credentials saved but connection test failed. Please verify and test again.",
                    )),
                )
                    .into_response()
            }
        },
        None => (
            StatusCode::OK,
            Json(ApiResponse::success("Credentials saved")),
        )
            .into_response(),
    }
}

/// POST /settings/meta/disconnect - Disconnect from Meta Commerce.
#[instrument(skip(state, session))]
async fn disconnect(State(state): State<AppState>, session: Session) -> Response {
    tracing::debug!("Processing Meta Commerce disconnect request");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to disconnect Meta Commerce");
        return response;
    }

    let repo = MetaCommerceCredentialsRepository::new(state.pool());
    if let Err(e) = repo.delete("default").await {
        tracing::error!(error = %e, "Failed to delete Meta Commerce credentials");
        return Redirect::to("/settings/meta?error=disconnect_failed").into_response();
    }

    tracing::info!("Successfully disconnected from Meta Commerce");
    Redirect::to("/settings/meta?success=disconnected").into_response()
}

/// POST /settings/meta/test - Test Meta Commerce connection.
#[instrument(skip(state, session))]
async fn test_connection(State(state): State<AppState>, session: Session) -> Response {
    tracing::debug!("Testing Meta Commerce connection");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to test Meta Commerce connection");
        return response;
    }

    let Some(client) = build_client_from_db(&state).await else {
        tracing::debug!("Meta Commerce test failed: no credentials configured");
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Meta Commerce is not connected")),
        )
            .into_response();
    };

    match client.test_connection().await {
        Ok(account_info) => {
            let repo = MetaCommerceCredentialsRepository::new(state.pool());
            if let Err(e) = repo.touch("default").await {
                tracing::warn!(error = %e, "Failed to update Meta Commerce last_used_at");
            }

            let account_name = account_info.name.as_deref().unwrap_or("Unknown");
            tracing::info!(
                account_name = %account_name,
                "Meta Commerce connection test passed"
            );
            (
                StatusCode::OK,
                Json(ApiResponse::success(format!("Connected: {account_name}"))),
            )
                .into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "Meta Commerce connection test failed");
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(format!("Connection test failed: {e}"))),
            )
                .into_response()
        }
    }
}

/// Build a `MetaCommerceClient` from stored DB credentials.
async fn build_client_from_db(
    state: &AppState,
) -> Option<naked_pineapple_services::meta_commerce::MetaCommerceClient> {
    use naked_pineapple_services::meta_commerce::MetaCommerceCredentials;

    let repo = MetaCommerceCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten()?;

    Some(
        naked_pineapple_services::meta_commerce::MetaCommerceClient::new(MetaCommerceCredentials {
            app_id: creds.app_id,
            app_secret: creds.app_secret,
            page_access_token: creds.page_access_token,
            page_id: creds.page_id,
            commerce_account_id: creds.commerce_account_id,
            catalog_id: creds.catalog_id,
        }),
    )
}

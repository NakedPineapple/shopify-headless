//! Pinterest settings routes.
//!
//! These routes handle the configuration of the Pinterest API integration
//! (Catalog Sync + Conversions API). Only `super_admin` users can manage
//! Pinterest settings.

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

use crate::db::{PinterestCredentialsRepository, SavePinterestParams};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::amazon_settings::ApiResponse;
use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Pinterest settings page template.
#[derive(Template)]
#[template(path = "settings/pinterest.html")]
pub struct PinterestSettingsTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub connected: bool,
    pub ad_account_id: Option<String>,
    pub last_used_at: Option<String>,
    pub success_message: Option<String>,
    pub error_message: Option<String>,
}

/// Build the Pinterest settings router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/settings/pinterest", get(settings_page))
        .route("/settings/pinterest/connect", post(connect))
        .route("/settings/pinterest/disconnect", post(disconnect))
        .route("/settings/pinterest/test", post(test_connection))
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

/// Request to connect Pinterest credentials.
#[derive(Deserialize)]
pub struct ConnectRequest {
    pub app_id: String,
    pub app_secret: String,
    pub access_token: String,
    pub refresh_token: String,
    pub ad_account_id: String,
}

impl std::fmt::Debug for ConnectRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectRequest")
            .field("app_id", &self.app_id)
            .field("app_secret", &"[REDACTED]")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("ad_account_id", &self.ad_account_id)
            .finish()
    }
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /settings/pinterest - Pinterest settings page.
#[instrument(skip(state, session))]
async fn settings_page(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<SettingsQueryParams>,
) -> Response {
    tracing::debug!("Loading Pinterest settings page");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to access Pinterest settings");
        return response;
    }

    let Some(admin) = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
    else {
        tracing::warn!("No admin in session for Pinterest settings page");
        return Redirect::to("/auth/login").into_response();
    };

    let repo = PinterestCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten();

    let (connected, ad_account_id, last_used_at) = if let Some(creds) = creds {
        let last_used = creds
            .last_used_at
            .map(|dt| dt.format("%b %d, %Y %H:%M UTC").to_string());

        (true, Some(creds.ad_account_id), last_used)
    } else {
        (false, None, None)
    };

    let success_message = params.success.as_deref().map(|s| match s {
        "connected" => "Successfully connected to Pinterest!".to_string(),
        "disconnected" => "Successfully disconnected from Pinterest.".to_string(),
        "test_passed" => "Connection test successful!".to_string(),
        _ => format!("Success: {s}"),
    });

    let error_message = params.error.as_deref().map(|e| match e {
        "auth_failed" => "Authentication failed. Please check your credentials.".to_string(),
        "disconnect_failed" => "Failed to disconnect from Pinterest.".to_string(),
        "test_failed" => "Connection test failed.".to_string(),
        "not_connected" => "Pinterest is not connected.".to_string(),
        _ => format!("Error: {e}"),
    });

    let template = PinterestSettingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/settings/pinterest".to_string(),
        connected,
        ad_account_id,
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

/// POST /settings/pinterest/connect - Connect Pinterest credentials.
#[instrument(skip(state, session, req))]
async fn connect(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<ConnectRequest>,
) -> Response {
    tracing::debug!("Attempting to connect Pinterest");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to connect Pinterest");
        return response;
    }

    let admin_id = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
        .map(|a| a.id.as_i32());

    if req.ad_account_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Ad Account ID is required")),
        )
            .into_response();
    }

    // Save credentials to database
    let repo = PinterestCredentialsRepository::new(state.pool());
    let params = SavePinterestParams {
        account_name: "default",
        app_id: req.app_id.trim(),
        app_secret: req.app_secret.trim(),
        access_token: req.access_token.trim(),
        refresh_token: req.refresh_token.trim(),
        ad_account_id: req.ad_account_id.trim(),
        connected_by: admin_id,
    };

    if let Err(e) = repo.save(&params).await {
        tracing::error!(error = %e, "Failed to save Pinterest credentials");
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
                let username = account_info.username.as_deref().unwrap_or("Unknown");
                tracing::info!(
                    ad_account_id = %req.ad_account_id,
                    username = %username,
                    "Successfully connected to Pinterest"
                );
                (
                    StatusCode::OK,
                    Json(ApiResponse::success(format!(
                        "Connected to Pinterest (@{username})"
                    ))),
                )
                    .into_response()
            }
            Err(e) => {
                tracing::warn!(error = %e, "Pinterest connection test failed after save");
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

/// POST /settings/pinterest/disconnect - Disconnect from Pinterest.
#[instrument(skip(state, session))]
async fn disconnect(State(state): State<AppState>, session: Session) -> Response {
    tracing::debug!("Processing Pinterest disconnect request");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to disconnect Pinterest");
        return response;
    }

    let repo = PinterestCredentialsRepository::new(state.pool());
    if let Err(e) = repo.delete("default").await {
        tracing::error!(error = %e, "Failed to delete Pinterest credentials");
        return Redirect::to("/settings/pinterest?error=disconnect_failed").into_response();
    }

    tracing::info!("Successfully disconnected from Pinterest");
    Redirect::to("/settings/pinterest?success=disconnected").into_response()
}

/// POST /settings/pinterest/test - Test Pinterest connection.
#[instrument(skip(state, session))]
async fn test_connection(State(state): State<AppState>, session: Session) -> Response {
    tracing::debug!("Testing Pinterest connection");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to test Pinterest connection");
        return response;
    }

    let Some(client) = build_client_from_db(&state).await else {
        tracing::debug!("Pinterest test failed: no credentials configured");
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Pinterest is not connected")),
        )
            .into_response();
    };

    match client.test_connection().await {
        Ok(account_info) => {
            let repo = PinterestCredentialsRepository::new(state.pool());
            if let Err(e) = repo.touch("default").await {
                tracing::warn!(error = %e, "Failed to update Pinterest last_used_at");
            }

            let username = account_info.username.as_deref().unwrap_or("Unknown");
            tracing::info!(
                username = %username,
                "Pinterest connection test passed"
            );
            (
                StatusCode::OK,
                Json(ApiResponse::success(format!("Connected: @{username}"))),
            )
                .into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "Pinterest connection test failed");
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(format!("Connection test failed: {e}"))),
            )
                .into_response()
        }
    }
}

/// Build a `PinterestClient` from stored DB credentials.
async fn build_client_from_db(
    state: &AppState,
) -> Option<naked_pineapple_services::pinterest::PinterestClient> {
    use naked_pineapple_services::pinterest::PinterestCredentials;

    let repo = PinterestCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten()?;

    Some(naked_pineapple_services::pinterest::PinterestClient::new(
        PinterestCredentials {
            app_id: creds.app_id,
            app_secret: creds.app_secret,
            access_token: creds.access_token,
            refresh_token: creds.refresh_token,
            ad_account_id: creds.ad_account_id,
        },
    ))
}

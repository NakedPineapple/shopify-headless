//! Faire settings routes.
//!
//! These routes handle the configuration of the Faire Brand API
//! integration (wholesale marketplace). Only `super_admin` users can
//! manage Faire settings.

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

use crate::db::{FaireCredentialsRepository, SaveFaireParams};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::amazon_settings::ApiResponse;
use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Faire settings page template.
#[derive(Template)]
#[template(path = "settings/faire.html")]
pub struct FaireSettingsTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub connected: bool,
    pub brand_id: Option<String>,
    pub last_used_at: Option<String>,
    pub success_message: Option<String>,
    pub error_message: Option<String>,
}

/// Build the Faire settings router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/settings/faire", get(settings_page))
        .route("/settings/faire/connect", post(connect))
        .route("/settings/faire/disconnect", post(disconnect))
        .route("/settings/faire/test", post(test_connection))
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

/// Request to connect Faire credentials.
#[derive(Deserialize)]
pub struct ConnectRequest {
    pub brand_id: String,
    pub api_token: String,
}

impl std::fmt::Debug for ConnectRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectRequest")
            .field("brand_id", &self.brand_id)
            .field("api_token", &"[REDACTED]")
            .finish()
    }
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /settings/faire - Faire settings page.
#[instrument(skip(state, session))]
async fn settings_page(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<SettingsQueryParams>,
) -> Response {
    tracing::debug!("Loading Faire settings page");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to access Faire settings");
        return response;
    }

    let Some(admin) = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
    else {
        tracing::warn!("No admin in session for Faire settings page");
        return Redirect::to("/auth/login").into_response();
    };

    let repo = FaireCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten();

    let (connected, brand_id, last_used_at) = if let Some(creds) = creds {
        let last_used = creds
            .last_used_at
            .map(|dt| dt.format("%b %d, %Y %H:%M UTC").to_string());

        (true, Some(creds.brand_id), last_used)
    } else {
        (false, None, None)
    };

    let success_message = params.success.as_deref().map(|s| match s {
        "connected" => "Successfully connected to Faire!".to_string(),
        "disconnected" => "Successfully disconnected from Faire.".to_string(),
        "test_passed" => "Connection test successful!".to_string(),
        _ => format!("Success: {s}"),
    });

    let error_message = params.error.as_deref().map(|e| match e {
        "auth_failed" => "Authentication failed. Please check your credentials.".to_string(),
        "disconnect_failed" => "Failed to disconnect from Faire.".to_string(),
        "test_failed" => "Connection test failed.".to_string(),
        "not_connected" => "Faire is not connected.".to_string(),
        _ => format!("Error: {e}"),
    });

    let template = FaireSettingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/settings/faire".to_string(),
        connected,
        brand_id,
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

/// POST /settings/faire/connect - Connect Faire credentials.
#[instrument(skip(state, session, req))]
async fn connect(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<ConnectRequest>,
) -> Response {
    tracing::debug!("Attempting to connect Faire");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to connect Faire");
        return response;
    }

    let admin_id = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
        .map(|a| a.id.as_i32());

    if req.brand_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Brand ID is required")),
        )
            .into_response();
    }

    // Save credentials to database
    let repo = FaireCredentialsRepository::new(state.pool());
    let params = SaveFaireParams {
        account_name: "default",
        brand_id: req.brand_id.trim(),
        api_token: req.api_token.trim(),
        connected_by: admin_id,
    };

    if let Err(e) = repo.save(&params).await {
        tracing::error!(error = %e, "Failed to save Faire credentials");
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
            Ok(brand_info) => {
                let name = brand_info.name.as_deref().unwrap_or("Unknown");
                tracing::info!(
                    brand_id = %req.brand_id,
                    brand_name = %name,
                    "Successfully connected to Faire"
                );
                (
                    StatusCode::OK,
                    Json(ApiResponse::success(format!("Connected to Faire ({name})"))),
                )
                    .into_response()
            }
            Err(e) => {
                tracing::warn!(error = %e, "Faire connection test failed after save");
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

/// POST /settings/faire/disconnect - Disconnect from Faire.
#[instrument(skip(state, session))]
async fn disconnect(State(state): State<AppState>, session: Session) -> Response {
    tracing::debug!("Processing Faire disconnect request");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to disconnect Faire");
        return response;
    }

    let repo = FaireCredentialsRepository::new(state.pool());
    if let Err(e) = repo.delete("default").await {
        tracing::error!(error = %e, "Failed to delete Faire credentials");
        return Redirect::to("/settings/faire?error=disconnect_failed").into_response();
    }

    tracing::info!("Successfully disconnected from Faire");
    Redirect::to("/settings/faire?success=disconnected").into_response()
}

/// POST /settings/faire/test - Test Faire connection.
#[instrument(skip(state, session))]
async fn test_connection(State(state): State<AppState>, session: Session) -> Response {
    tracing::debug!("Testing Faire connection");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to test Faire connection");
        return response;
    }

    let Some(client) = build_client_from_db(&state).await else {
        tracing::debug!("Faire test failed: no credentials configured");
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Faire is not connected")),
        )
            .into_response();
    };

    match client.test_connection().await {
        Ok(brand_info) => {
            let repo = FaireCredentialsRepository::new(state.pool());
            if let Err(e) = repo.touch("default").await {
                tracing::warn!(error = %e, "Failed to update Faire last_used_at");
            }

            let name = brand_info.name.as_deref().unwrap_or("Unknown");
            tracing::info!(
                brand_name = %name,
                "Faire connection test passed"
            );
            (
                StatusCode::OK,
                Json(ApiResponse::success(format!("Connected: {name}"))),
            )
                .into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "Faire connection test failed");
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(format!("Connection test failed: {e}"))),
            )
                .into_response()
        }
    }
}

/// Build a `FaireClient` from stored DB credentials.
async fn build_client_from_db(
    state: &AppState,
) -> Option<naked_pineapple_services::faire::FaireClient> {
    use naked_pineapple_services::faire::FaireCredentials as ApiCredentials;

    let repo = FaireCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten()?;

    Some(naked_pineapple_services::faire::FaireClient::new(
        ApiCredentials {
            brand_id: creds.brand_id,
            api_token: creds.api_token,
        },
    ))
}

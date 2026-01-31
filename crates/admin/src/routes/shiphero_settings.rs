//! `ShipHero` settings routes.
//!
//! These routes handle the configuration of `ShipHero` warehouse integration.
//! Only `super_admin` users can manage `ShipHero` settings.

use askama::Template;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use tracing::instrument;

use crate::db::{SaveCredentialsParams, ShipHeroCredentialsRepository};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::shiphero::ShipHeroClient;
use crate::state::AppState;

use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// `ShipHero` settings page template.
#[derive(Template)]
#[template(path = "settings/shiphero.html")]
pub struct ShipHeroSettingsTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub connected: bool,
    pub email: Option<String>,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub success_message: Option<String>,
    pub error_message: Option<String>,
}

/// Build the `ShipHero` settings router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/settings/shiphero", get(settings_page))
        .route("/settings/shiphero/connect", post(connect))
        .route("/settings/shiphero/disconnect", post(disconnect))
        .route("/settings/shiphero/test", post(test_connection))
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

/// Request to connect to `ShipHero`.
#[derive(Debug, Deserialize)]
pub struct ConnectRequest {
    pub email: String,
    pub password: String,
}

/// API response.
#[derive(Debug, Serialize)]
pub struct ApiResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ApiResponse {
    fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: Some(message.into()),
            error: None,
        }
    }

    fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            message: None,
            error: Some(error.into()),
        }
    }
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /settings/shiphero - `ShipHero` settings page.
///
/// Shows the current connection status and allows connecting/disconnecting.
#[instrument(skip(state, session))]
async fn settings_page(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<SettingsQueryParams>,
) -> Response {
    // Check super_admin permission
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    // Get current admin from session
    let Some(admin) = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
    else {
        return Redirect::to("/auth/login").into_response();
    };

    // Check connection status
    let repo = ShipHeroCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten();

    let (connected, email, expires_at, last_used_at) = if let Some(creds) = creds {
        let now = chrono::Utc::now().timestamp();
        let is_valid = now < creds.access_token_expires_at - 60;

        let expires_at = chrono::DateTime::from_timestamp(creds.access_token_expires_at, 0)
            .map(|dt| dt.format("%b %d, %Y %H:%M UTC").to_string());

        let last_used = creds
            .last_used_at
            .map(|dt| dt.format("%b %d, %Y %H:%M UTC").to_string());

        (is_valid, Some(creds.email), expires_at, last_used)
    } else {
        (false, None, None, None)
    };

    // Map query params to user-friendly messages
    let success_message = params.success.as_deref().map(|s| match s {
        "connected" => "Successfully connected to ShipHero!".to_string(),
        "disconnected" => "Successfully disconnected from ShipHero.".to_string(),
        "test_passed" => "Connection test successful!".to_string(),
        _ => format!("Success: {s}"),
    });

    let error_message = params.error.as_deref().map(|e| match e {
        "auth_failed" => "Authentication failed. Please check your credentials.".to_string(),
        "disconnect_failed" => "Failed to disconnect from ShipHero.".to_string(),
        "test_failed" => "Connection test failed.".to_string(),
        "not_connected" => "ShipHero is not connected.".to_string(),
        _ => format!("Error: {e}"),
    });

    let template = ShipHeroSettingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/settings/shiphero".to_string(),
        connected,
        email,
        expires_at,
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

/// POST /settings/shiphero/connect - Connect to `ShipHero`.
///
/// Authenticates with email/password to obtain JWT tokens.
#[instrument(skip(state, session, req))]
async fn connect(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<ConnectRequest>,
) -> Response {
    // Check super_admin permission
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    // Get current admin for audit trail
    let admin_id = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
        .map(|a| a.id.as_i32());

    // Validate input
    let email = req.email.trim();
    if email.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Email is required")),
        )
            .into_response();
    }

    let password = req.password.trim();
    if password.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Password is required")),
        )
            .into_response();
    }

    // Create a temporary client and authenticate
    let client = ShipHeroClient::new();
    let password_secret = SecretString::from(password.to_string());

    let token = match client.authenticate(email, &password_secret).await {
        Ok(token) => token,
        Err(e) => {
            tracing::error!(error = %e, "ShipHero authentication failed");
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::error(format!("Authentication failed: {e}"))),
            )
                .into_response();
        }
    };

    // Save credentials to database
    let repo = ShipHeroCredentialsRepository::new(state.pool());
    let params = SaveCredentialsParams {
        account_name: "default",
        email,
        access_token: token.access_token.expose_secret(),
        refresh_token: token
            .refresh_token
            .as_ref()
            .map(secrecy::ExposeSecret::expose_secret),
        access_token_expires_at: token.access_token_expires_at,
        refresh_token_expires_at: token.refresh_token_expires_at,
        connected_by: admin_id,
    };

    if let Err(e) = repo.save(&params).await {
        tracing::error!(error = %e, "Failed to save ShipHero credentials");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error("Failed to save credentials")),
        )
            .into_response();
    }

    tracing::info!(email = %email, "Successfully connected to ShipHero");
    (
        StatusCode::OK,
        Json(ApiResponse::success("Connected to ShipHero")),
    )
        .into_response()
}

/// POST /settings/shiphero/disconnect - Disconnect from `ShipHero`.
#[instrument(skip(state, session))]
async fn disconnect(State(state): State<AppState>, session: Session) -> Response {
    // Check super_admin permission
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    // Delete credentials from database
    let repo = ShipHeroCredentialsRepository::new(state.pool());
    if let Err(e) = repo.delete("default").await {
        tracing::error!(error = %e, "Failed to delete ShipHero credentials");
        return Redirect::to("/settings/shiphero?error=disconnect_failed").into_response();
    }

    tracing::info!("Disconnected from ShipHero");
    Redirect::to("/settings/shiphero?success=disconnected").into_response()
}

/// POST /settings/shiphero/test - Test connection to `ShipHero`.
#[instrument(skip(state, session))]
async fn test_connection(State(state): State<AppState>, session: Session) -> Response {
    // Check super_admin permission
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    // Check if we have a client configured
    let Some(client) = state.shiphero() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("ShipHero is not connected")),
        )
            .into_response();
    };

    // Test the connection
    match client.test_connection().await {
        Ok(account) => {
            // Update last_used_at
            let repo = ShipHeroCredentialsRepository::new(state.pool());
            let _ = repo.touch("default").await;

            tracing::info!(account_id = %account.id, "ShipHero connection test passed");
            (
                StatusCode::OK,
                Json(ApiResponse::success(format!(
                    "Connected as: {}",
                    account.email
                ))),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "ShipHero connection test failed");
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(format!("Connection test failed: {e}"))),
            )
                .into_response()
        }
    }
}

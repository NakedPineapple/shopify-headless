//! Google Merchant Center settings routes.
//!
//! These routes handle the configuration of the Google Merchant Center API
//! integration. Only `super_admin` users can manage Google settings.

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

use crate::db::{GoogleCredentialsRepository, SaveGoogleParams};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::amazon_settings::ApiResponse;
use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Google settings page template.
#[derive(Template)]
#[template(path = "settings/google.html")]
pub struct GoogleSettingsTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub connected: bool,
    pub merchant_id: Option<String>,
    pub last_used_at: Option<String>,
    pub success_message: Option<String>,
    pub error_message: Option<String>,
}

/// Build the Google settings router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/settings/google", get(settings_page))
        .route("/settings/google/connect", post(connect))
        .route("/settings/google/disconnect", post(disconnect))
        .route("/settings/google/test", post(test_connection))
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

/// Request to connect Google Merchant Center credentials.
#[derive(Deserialize)]
pub struct ConnectRequest {
    pub merchant_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub access_token: String,
    pub refresh_token: String,
}

impl std::fmt::Debug for ConnectRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectRequest")
            .field("merchant_id", &self.merchant_id)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .finish()
    }
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /settings/google - Google settings page.
#[instrument(skip(state, session))]
async fn settings_page(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<SettingsQueryParams>,
) -> Response {
    tracing::debug!("Loading Google settings page");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to access Google settings");
        return response;
    }

    let Some(admin) = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
    else {
        tracing::warn!("No admin in session for Google settings page");
        return Redirect::to("/auth/login").into_response();
    };

    let repo = GoogleCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten();

    let (connected, merchant_id, last_used_at) = if let Some(creds) = creds {
        let last_used = creds
            .last_used_at
            .map(|dt| dt.format("%b %d, %Y %H:%M UTC").to_string());

        (true, Some(creds.merchant_id), last_used)
    } else {
        (false, None, None)
    };

    let success_message = params.success.as_deref().map(|s| match s {
        "connected" => "Successfully connected to Google Merchant Center!".to_string(),
        "disconnected" => "Successfully disconnected from Google Merchant Center.".to_string(),
        "test_passed" => "Connection test successful!".to_string(),
        _ => format!("Success: {s}"),
    });

    let error_message = params.error.as_deref().map(|e| match e {
        "auth_failed" => "Authentication failed. Please check your credentials.".to_string(),
        "disconnect_failed" => "Failed to disconnect from Google Merchant Center.".to_string(),
        "test_failed" => "Connection test failed.".to_string(),
        "not_connected" => "Google Merchant Center is not connected.".to_string(),
        _ => format!("Error: {e}"),
    });

    let template = GoogleSettingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/settings/google".to_string(),
        connected,
        merchant_id,
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

/// POST /settings/google/connect - Connect Google credentials.
#[instrument(skip(state, session, req))]
async fn connect(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<ConnectRequest>,
) -> Response {
    tracing::debug!("Attempting to connect Google Merchant Center");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to connect Google");
        return response;
    }

    let admin_id = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
        .map(|a| a.id.as_i32());

    if req.merchant_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Merchant Center ID is required")),
        )
            .into_response();
    }

    // Save credentials to database
    let repo = GoogleCredentialsRepository::new(state.pool());
    let params = SaveGoogleParams {
        account_name: "default",
        merchant_id: req.merchant_id.trim(),
        client_id: req.client_id.trim(),
        client_secret: req.client_secret.trim(),
        access_token: req.access_token.trim(),
        refresh_token: req.refresh_token.trim(),
        connected_by: admin_id,
    };

    if let Err(e) = repo.save(&params).await {
        tracing::error!(error = %e, "Failed to save Google credentials");
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
                let name = account_info.name.as_deref().unwrap_or("Unknown");
                tracing::info!(
                    merchant_id = %req.merchant_id,
                    name = %name,
                    "Successfully connected to Google Merchant Center"
                );
                (
                    StatusCode::OK,
                    Json(ApiResponse::success(format!(
                        "Connected to Google Merchant Center ({name})"
                    ))),
                )
                    .into_response()
            }
            Err(e) => {
                tracing::warn!(error = %e, "Google connection test failed after save");
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

/// POST /settings/google/disconnect - Disconnect from Google.
#[instrument(skip(state, session))]
async fn disconnect(State(state): State<AppState>, session: Session) -> Response {
    tracing::debug!("Processing Google disconnect request");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to disconnect Google");
        return response;
    }

    let repo = GoogleCredentialsRepository::new(state.pool());
    if let Err(e) = repo.delete("default").await {
        tracing::error!(error = %e, "Failed to delete Google credentials");
        return Redirect::to("/settings/google?error=disconnect_failed").into_response();
    }

    tracing::info!("Successfully disconnected from Google Merchant Center");
    Redirect::to("/settings/google?success=disconnected").into_response()
}

/// POST /settings/google/test - Test Google connection.
#[instrument(skip(state, session))]
async fn test_connection(State(state): State<AppState>, session: Session) -> Response {
    tracing::debug!("Testing Google connection");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to test Google connection");
        return response;
    }

    let Some(client) = build_client_from_db(&state).await else {
        tracing::debug!("Google test failed: no credentials configured");
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(
                "Google Merchant Center is not connected",
            )),
        )
            .into_response();
    };

    match client.test_connection().await {
        Ok(account_info) => {
            let repo = GoogleCredentialsRepository::new(state.pool());
            if let Err(e) = repo.touch("default").await {
                tracing::warn!(error = %e, "Failed to update Google last_used_at");
            }

            let name = account_info.name.as_deref().unwrap_or("Unknown");
            tracing::info!(
                name = %name,
                "Google connection test passed"
            );
            (
                StatusCode::OK,
                Json(ApiResponse::success(format!("Connected: {name}"))),
            )
                .into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "Google connection test failed");
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(format!("Connection test failed: {e}"))),
            )
                .into_response()
        }
    }
}

/// Build a `GoogleMerchantClient` from stored DB credentials.
async fn build_client_from_db(
    state: &AppState,
) -> Option<naked_pineapple_services::google_merchant::GoogleMerchantClient> {
    use naked_pineapple_services::google_merchant::GoogleMerchantCredentials;

    let repo = GoogleCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten()?;

    Some(
        naked_pineapple_services::google_merchant::GoogleMerchantClient::new(
            GoogleMerchantCredentials {
                merchant_id: creds.merchant_id,
                client_id: creds.client_id,
                client_secret: creds.client_secret,
                access_token: creds.access_token,
                refresh_token: creds.refresh_token,
            },
        ),
    )
}

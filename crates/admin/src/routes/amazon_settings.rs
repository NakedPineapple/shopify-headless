//! Amazon SP-API settings routes.
//!
//! These routes handle the configuration of the Amazon Selling Partner API
//! integration. Only `super_admin` users can manage Amazon SP-API settings.

use askama::Template;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use tracing::instrument;

use crate::db::{AmazonSpCredentialsRepository, SaveAmazonSpParams};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Amazon SP-API settings page template.
#[derive(Template)]
#[template(path = "settings/amazon.html")]
pub struct AmazonSettingsTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub connected: bool,
    pub seller_id: Option<String>,
    pub marketplace_id: Option<String>,
    pub last_used_at: Option<String>,
    pub success_message: Option<String>,
    pub error_message: Option<String>,
}

/// Build the Amazon SP-API settings router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/settings/amazon", get(settings_page))
        .route("/settings/amazon/connect", post(connect))
        .route("/settings/amazon/disconnect", post(disconnect))
        .route("/settings/amazon/test", post(test_connection))
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

/// Request to connect Amazon SP-API credentials.
#[derive(Deserialize)]
pub struct ConnectRequest {
    pub lwa_client_id: String,
    pub lwa_client_secret: String,
    pub lwa_refresh_token: String,
    pub aws_access_key_id: String,
    pub aws_secret_access_key: String,
    pub seller_id: String,
}

impl std::fmt::Debug for ConnectRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectRequest")
            .field("lwa_client_id", &self.lwa_client_id)
            .field("lwa_client_secret", &"[REDACTED]")
            .field("lwa_refresh_token", &"[REDACTED]")
            .field("aws_access_key_id", &self.aws_access_key_id)
            .field("aws_secret_access_key", &"[REDACTED]")
            .field("seller_id", &self.seller_id)
            .finish()
    }
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
    pub(crate) fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: Some(message.into()),
            error: None,
        }
    }

    pub(crate) fn error(error: impl Into<String>) -> Self {
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

/// GET /settings/amazon - Amazon SP-API settings page.
#[instrument(skip(state, session))]
async fn settings_page(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<SettingsQueryParams>,
) -> Response {
    tracing::debug!("Loading Amazon SP-API settings page");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to access Amazon settings");
        return response;
    }

    let Some(admin) = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
    else {
        tracing::warn!("No admin in session for Amazon settings page");
        return Redirect::to("/auth/login").into_response();
    };

    let repo = AmazonSpCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten();

    let (connected, seller_id, marketplace_id, last_used_at) = if let Some(creds) = creds {
        let last_used = creds
            .last_used_at
            .map(|dt| dt.format("%b %d, %Y %H:%M UTC").to_string());

        (
            true,
            Some(creds.seller_id),
            Some(creds.marketplace_id),
            last_used,
        )
    } else {
        (false, None, None, None)
    };

    let success_message = params.success.as_deref().map(|s| match s {
        "connected" => "Successfully connected to Amazon SP-API!".to_string(),
        "disconnected" => "Successfully disconnected from Amazon SP-API.".to_string(),
        "test_passed" => "Connection test successful!".to_string(),
        _ => format!("Success: {s}"),
    });

    let error_message = params.error.as_deref().map(|e| match e {
        "auth_failed" => "Authentication failed. Please check your credentials.".to_string(),
        "disconnect_failed" => "Failed to disconnect from Amazon SP-API.".to_string(),
        "test_failed" => "Connection test failed.".to_string(),
        "not_connected" => "Amazon SP-API is not connected.".to_string(),
        _ => format!("Error: {e}"),
    });

    let template = AmazonSettingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/settings/amazon".to_string(),
        connected,
        seller_id,
        marketplace_id,
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

/// POST /settings/amazon/connect - Connect Amazon SP-API credentials.
#[instrument(skip(state, session, req))]
async fn connect(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<ConnectRequest>,
) -> Response {
    tracing::debug!("Attempting to connect Amazon SP-API");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to connect Amazon SP-API");
        return response;
    }

    let admin_id = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
        .map(|a| a.id.as_i32());

    if req.seller_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Seller ID is required")),
        )
            .into_response();
    }

    // Save credentials to database
    let repo = AmazonSpCredentialsRepository::new(state.pool());
    let params = SaveAmazonSpParams {
        account_name: "default",
        lwa_client_id: req.lwa_client_id.trim(),
        lwa_client_secret: req.lwa_client_secret.trim(),
        lwa_refresh_token: req.lwa_refresh_token.trim(),
        aws_access_key_id: req.aws_access_key_id.trim(),
        aws_secret_access_key: req.aws_secret_access_key.trim(),
        seller_id: req.seller_id.trim(),
        connected_by: admin_id,
    };

    if let Err(e) = repo.save(&params).await {
        tracing::error!(error = %e, "Failed to save Amazon SP-API credentials");
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
            Ok(participations) => {
                let market_count = participations.len();
                tracing::info!(
                    seller_id = %req.seller_id,
                    marketplaces = market_count,
                    "Successfully connected to Amazon SP-API"
                );
                (
                    StatusCode::OK,
                    Json(ApiResponse::success(format!(
                        "Connected to Amazon SP-API ({market_count} marketplace(s))"
                    ))),
                )
                    .into_response()
            }
            Err(e) => {
                tracing::warn!(error = %e, "Amazon SP-API connection test failed after save");
                // Credentials saved but test failed — keep them so user can fix
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

/// POST /settings/amazon/disconnect - Disconnect from Amazon SP-API.
#[instrument(skip(state, session))]
async fn disconnect(State(state): State<AppState>, session: Session) -> Response {
    tracing::debug!("Processing Amazon SP-API disconnect request");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to disconnect Amazon SP-API");
        return response;
    }

    let repo = AmazonSpCredentialsRepository::new(state.pool());
    if let Err(e) = repo.delete("default").await {
        tracing::error!(error = %e, "Failed to delete Amazon SP-API credentials");
        return Redirect::to("/settings/amazon?error=disconnect_failed").into_response();
    }

    tracing::info!("Successfully disconnected from Amazon SP-API");
    Redirect::to("/settings/amazon?success=disconnected").into_response()
}

/// POST /settings/amazon/test - Test Amazon SP-API connection.
#[instrument(skip(state, session))]
async fn test_connection(State(state): State<AppState>, session: Session) -> Response {
    tracing::debug!("Testing Amazon SP-API connection");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to test Amazon SP-API connection");
        return response;
    }

    let Some(client) = build_client_from_db(&state).await else {
        tracing::debug!("Amazon SP-API test failed: no credentials configured");
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Amazon SP-API is not connected")),
        )
            .into_response();
    };

    match client.test_connection().await {
        Ok(participations) => {
            let repo = AmazonSpCredentialsRepository::new(state.pool());
            if let Err(e) = repo.touch("default").await {
                tracing::warn!(error = %e, "Failed to update Amazon SP-API last_used_at");
            }

            let market_count = participations.len();
            tracing::info!(
                marketplaces = market_count,
                "Amazon SP-API connection test passed"
            );
            (
                StatusCode::OK,
                Json(ApiResponse::success(format!(
                    "Connected: {market_count} marketplace(s) active"
                ))),
            )
                .into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "Amazon SP-API connection test failed");
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(format!("Connection test failed: {e}"))),
            )
                .into_response()
        }
    }
}

/// Build an `AmazonSpClient` from stored DB credentials.
async fn build_client_from_db(
    state: &AppState,
) -> Option<naked_pineapple_services::amazon_sp::AmazonSpClient> {
    use naked_pineapple_services::amazon_sp::AmazonCredentials;

    let repo = AmazonSpCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten()?;

    Some(naked_pineapple_services::amazon_sp::AmazonSpClient::new(
        AmazonCredentials {
            lwa_client_id: creds.lwa_client_id,
            lwa_client_secret: creds.lwa_client_secret,
            lwa_refresh_token: creds.lwa_refresh_token,
            aws_access_key_id: creds.aws_access_key_id,
            aws_secret_access_key: creds.aws_secret_access_key,
            seller_id: creds.seller_id,
            marketplace_id: creds.marketplace_id,
        },
    ))
}

//! TikTok Shop settings routes.
//!
//! These routes handle the configuration of the TikTok Shop API
//! integration. Only `super_admin` users can manage TikTok Shop settings.

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

use crate::db::{SaveTikTokShopParams, TikTokShopCredentialsRepository};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::amazon_settings::ApiResponse;
use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// TikTok Shop settings page template.
#[derive(Template)]
#[template(path = "settings/tiktok.html")]
pub struct TikTokSettingsTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub connected: bool,
    pub shop_id: Option<String>,
    pub shop_cipher: Option<String>,
    pub last_used_at: Option<String>,
    pub success_message: Option<String>,
    pub error_message: Option<String>,
}

/// Build the TikTok Shop settings router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/settings/tiktok", get(settings_page))
        .route("/settings/tiktok/connect", post(connect))
        .route("/settings/tiktok/disconnect", post(disconnect))
        .route("/settings/tiktok/test", post(test_connection))
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

/// Request to connect TikTok Shop credentials.
#[derive(Deserialize)]
pub struct ConnectRequest {
    pub app_key: String,
    pub app_secret: String,
    pub access_token: String,
    pub refresh_token: String,
    pub shop_id: String,
    pub shop_cipher: String,
}

impl std::fmt::Debug for ConnectRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectRequest")
            .field("app_key", &self.app_key)
            .field("app_secret", &"[REDACTED]")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("shop_id", &self.shop_id)
            .field("shop_cipher", &self.shop_cipher)
            .finish()
    }
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /settings/tiktok - TikTok Shop settings page.
#[instrument(skip(state, session))]
async fn settings_page(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<SettingsQueryParams>,
) -> Response {
    tracing::debug!("Loading TikTok Shop settings page");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to access TikTok Shop settings");
        return response;
    }

    let Some(admin) = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
    else {
        tracing::warn!("No admin in session for TikTok Shop settings page");
        return Redirect::to("/auth/login").into_response();
    };

    let repo = TikTokShopCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten();

    let (connected, shop_id, shop_cipher, last_used_at) = if let Some(creds) = creds {
        let last_used = creds
            .last_used_at
            .map(|dt| dt.format("%b %d, %Y %H:%M UTC").to_string());

        (
            true,
            Some(creds.shop_id),
            Some(creds.shop_cipher),
            last_used,
        )
    } else {
        (false, None, None, None)
    };

    let success_message = params.success.as_deref().map(|s| match s {
        "connected" => "Successfully connected to TikTok Shop!".to_string(),
        "disconnected" => "Successfully disconnected from TikTok Shop.".to_string(),
        "test_passed" => "Connection test successful!".to_string(),
        _ => format!("Success: {s}"),
    });

    let error_message = params.error.as_deref().map(|e| match e {
        "auth_failed" => "Authentication failed. Please check your credentials.".to_string(),
        "disconnect_failed" => "Failed to disconnect from TikTok Shop.".to_string(),
        "test_failed" => "Connection test failed.".to_string(),
        "not_connected" => "TikTok Shop is not connected.".to_string(),
        _ => format!("Error: {e}"),
    });

    let template = TikTokSettingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/settings/tiktok".to_string(),
        connected,
        shop_id,
        shop_cipher,
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

/// POST /settings/tiktok/connect - Connect TikTok Shop credentials.
#[instrument(skip(state, session, req))]
async fn connect(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<ConnectRequest>,
) -> Response {
    tracing::debug!("Attempting to connect TikTok Shop");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to connect TikTok Shop");
        return response;
    }

    let admin_id = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
        .map(|a| a.id.as_i32());

    if req.shop_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Shop ID is required")),
        )
            .into_response();
    }

    let repo = TikTokShopCredentialsRepository::new(state.pool());
    let params = SaveTikTokShopParams {
        account_name: "default",
        app_key: req.app_key.trim(),
        app_secret: req.app_secret.trim(),
        access_token: req.access_token.trim(),
        refresh_token: req.refresh_token.trim(),
        shop_id: req.shop_id.trim(),
        shop_cipher: req.shop_cipher.trim(),
        connected_by: admin_id,
    };

    if let Err(e) = repo.save(&params).await {
        tracing::error!(error = %e, "Failed to save TikTok Shop credentials");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error("Failed to save credentials")),
        )
            .into_response();
    }

    // Test the connection with the new credentials
    let client = build_client_from_db(&state).await;
    match client {
        Some(c) => match c.get_shop_info().await {
            Ok(shop_info) => {
                let shop_name = shop_info
                    .shops
                    .as_ref()
                    .and_then(|s| s.first())
                    .and_then(|s| s.name.as_deref())
                    .unwrap_or("Unknown");
                tracing::info!(
                    shop_id = %req.shop_id,
                    shop_name = %shop_name,
                    "Successfully connected to TikTok Shop"
                );
                (
                    StatusCode::OK,
                    Json(ApiResponse::success(format!(
                        "Connected to TikTok Shop ({shop_name})"
                    ))),
                )
                    .into_response()
            }
            Err(e) => {
                tracing::warn!(error = %e, "TikTok Shop connection test failed after save");
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

/// POST /settings/tiktok/disconnect - Disconnect from TikTok Shop.
#[instrument(skip(state, session))]
async fn disconnect(State(state): State<AppState>, session: Session) -> Response {
    tracing::debug!("Processing TikTok Shop disconnect request");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to disconnect TikTok Shop");
        return response;
    }

    let repo = TikTokShopCredentialsRepository::new(state.pool());
    if let Err(e) = repo.delete("default").await {
        tracing::error!(error = %e, "Failed to delete TikTok Shop credentials");
        return Redirect::to("/settings/tiktok?error=disconnect_failed").into_response();
    }

    tracing::info!("Successfully disconnected from TikTok Shop");
    Redirect::to("/settings/tiktok?success=disconnected").into_response()
}

/// POST /settings/tiktok/test - Test TikTok Shop connection.
#[instrument(skip(state, session))]
async fn test_connection(State(state): State<AppState>, session: Session) -> Response {
    tracing::debug!("Testing TikTok Shop connection");

    if let Err(response) = require_super_admin(&state, &session).await {
        tracing::warn!("Non-super-admin attempted to test TikTok Shop connection");
        return response;
    }

    let Some(client) = build_client_from_db(&state).await else {
        tracing::debug!("TikTok Shop test failed: no credentials configured");
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("TikTok Shop is not connected")),
        )
            .into_response();
    };

    match client.get_shop_info().await {
        Ok(shop_info) => {
            let repo = TikTokShopCredentialsRepository::new(state.pool());
            if let Err(e) = repo.touch("default").await {
                tracing::warn!(error = %e, "Failed to update TikTok Shop last_used_at");
            }

            let shop_name = shop_info
                .shops
                .as_ref()
                .and_then(|s| s.first())
                .and_then(|s| s.name.as_deref())
                .unwrap_or("Unknown");
            tracing::info!(
                shop_name = %shop_name,
                "TikTok Shop connection test passed"
            );
            (
                StatusCode::OK,
                Json(ApiResponse::success(format!("Connected: {shop_name}"))),
            )
                .into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "TikTok Shop connection test failed");
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(format!("Connection test failed: {e}"))),
            )
                .into_response()
        }
    }
}

/// Build a `TikTokShopClient` from stored DB credentials.
async fn build_client_from_db(
    state: &AppState,
) -> Option<naked_pineapple_services::tiktok_shop::TikTokShopClient> {
    use naked_pineapple_services::tiktok_shop::TikTokShopCredentials as ApiCredentials;
    use secrecy::ExposeSecret;

    let repo = TikTokShopCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten()?;

    Some(
        naked_pineapple_services::tiktok_shop::TikTokShopClient::new(ApiCredentials {
            app_key: creds.app_key,
            app_secret: secrecy::SecretString::from(creds.app_secret.expose_secret().to_string()),
            access_token: secrecy::SecretString::from(
                creds.access_token.expose_secret().to_string(),
            ),
            refresh_token: secrecy::SecretString::from(
                creds.refresh_token.expose_secret().to_string(),
            ),
            shop_id: creds.shop_id,
            shop_cipher: creds.shop_cipher,
        }),
    )
}

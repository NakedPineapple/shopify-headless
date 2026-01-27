//! Shopify Admin API OAuth routes.
//!
//! These routes handle the OAuth flow to connect the admin panel to Shopify's Admin API.
//! Only `super_admin` users can manage Shopify settings.

use askama::Template;
use axum::{
    Router,
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use tower_sessions::Session;
use tracing::instrument;

use crate::db::ShopifyTokenRepository;
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::dashboard::AdminUserView;

const OAUTH_STATE_KEY: &str = "shopify_admin_oauth_state";

/// Required scopes for the Admin API.
const ADMIN_SCOPES: &[&str] = &[
    "read_products",
    "write_products",
    "read_orders",
    "write_orders",
    "read_inventory",
    "write_inventory",
    "read_customers",
    "read_fulfillments",
    "write_fulfillments",
    "read_publications",
    "read_reports",
    "read_marketing_activities",
    "read_shopify_payments_payouts",
];

// =============================================================================
// Templates
// =============================================================================

/// Shopify settings page template.
#[derive(Template)]
#[template(path = "shopify/settings.html")]
pub struct ShopifySettingsTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub connected: bool,
    pub shop: String,
    pub scopes: Vec<String>,
    pub success_message: Option<String>,
    pub error_message: Option<String>,
}

/// Build the Shopify OAuth router.
///
/// Routes that require authentication use the `require_super_admin` middleware.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/shopify", get(settings_page))
        .route("/shopify/connect", get(connect))
        .route("/shopify/disconnect", get(disconnect))
        .route("/shopify/callback", get(callback))
}

// =============================================================================
// Query Parameters
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct SettingsQueryParams {
    pub success: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OAuthCallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub shop: Option<String>,
    pub hmac: Option<String>,
    pub timestamp: Option<String>,
    pub host: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

// =============================================================================
// HMAC Verification
// =============================================================================

type HmacSha256 = Hmac<Sha256>;

/// Verify the HMAC signature from Shopify OAuth callback.
fn verify_shopify_hmac(params: &OAuthCallbackParams, client_secret: &str) -> bool {
    let Some(provided_hmac) = &params.hmac else {
        return false;
    };

    // Build the message from sorted params (excluding hmac and signature)
    let mut param_pairs: Vec<(String, String)> = Vec::new();

    if let Some(v) = &params.code {
        param_pairs.push(("code".to_string(), v.clone()));
    }
    if let Some(v) = &params.host {
        param_pairs.push(("host".to_string(), v.clone()));
    }
    if let Some(v) = &params.shop {
        param_pairs.push(("shop".to_string(), v.clone()));
    }
    if let Some(v) = &params.state {
        param_pairs.push(("state".to_string(), v.clone()));
    }
    if let Some(v) = &params.timestamp {
        param_pairs.push(("timestamp".to_string(), v.clone()));
    }

    // Sort alphabetically by key
    param_pairs.sort_by(|a, b| a.0.cmp(&b.0));

    // Build the message string
    let message: String = param_pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");

    // Compute HMAC-SHA256
    let Ok(mut mac) = HmacSha256::new_from_slice(client_secret.as_bytes()) else {
        return false;
    };
    mac.update(message.as_bytes());

    let computed = hex::encode(mac.finalize().into_bytes());

    // Constant-time comparison
    computed == *provided_hmac
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /shopify - Shopify settings page.
///
/// Shows the current connection status and allows connecting/disconnecting.
#[instrument(skip(state, session))]
async fn settings_page(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<SettingsQueryParams>,
) -> Response {
    // Check super_admin permission
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    // Get current admin from session (we know it exists after require_super_admin)
    let Some(admin) = session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
    else {
        // This should never happen after require_super_admin, but handle gracefully
        return Redirect::to("/auth/login").into_response();
    };

    // Check connection status and get token details
    let shop = state.shopify().store().to_string();
    let repo = ShopifyTokenRepository::new(state.pool());
    let token = repo.get_by_shop(&shop).await.ok().flatten();
    let connected = token.is_some();
    let scopes = token.map_or_else(Vec::new, |t| t.scopes);

    // Map query params to user-friendly messages
    let success_message = params.success.as_deref().map(|s| match s {
        "connected" => "Successfully connected to Shopify!".to_string(),
        "disconnected" => "Successfully disconnected from Shopify.".to_string(),
        _ => format!("Success: {s}"),
    });

    let error_message = params.error.as_deref().map(|e| match e {
        "oauth_denied" => "OAuth authorization was denied.".to_string(),
        "oauth_invalid_hmac" => "Invalid security signature. Please try again.".to_string(),
        "oauth_invalid_state" => "Invalid state parameter. Please try again.".to_string(),
        "oauth_failed" => "OAuth flow failed. Please try again.".to_string(),
        "oauth_exchange_failed" => "Failed to exchange authorization code.".to_string(),
        "oauth_save_failed" => "Failed to save credentials.".to_string(),
        "disconnect_failed" => "Failed to disconnect from Shopify.".to_string(),
        _ => format!("Error: {e}"),
    });

    let template = ShopifySettingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/shopify".to_string(),
        connected,
        shop,
        scopes,
        success_message,
        error_message,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

/// GET /shopify/connect - Start OAuth flow.
#[instrument(skip(state, session))]
async fn connect(State(state): State<AppState>, session: Session) -> Response {
    // Check super_admin permission
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    // Generate a random state parameter for CSRF protection
    let oauth_state = uuid::Uuid::new_v4().to_string();

    // Store state in session
    if let Err(e) = session.insert(OAUTH_STATE_KEY, &oauth_state).await {
        tracing::error!("Failed to store OAuth state: {}", e);
        return Redirect::to("/shopify?error=oauth_failed").into_response();
    }

    // Build redirect URI
    let redirect_uri = format!("{}/shopify/callback", state.config().base_url);

    // Generate authorization URL
    let auth_url = state
        .shopify()
        .authorization_url(&redirect_uri, ADMIN_SCOPES, &oauth_state);

    tracing::info!("Redirecting to Shopify OAuth: {}", auth_url);
    Redirect::to(&auth_url).into_response()
}

/// GET /shopify/callback - Handle OAuth callback.
#[instrument(skip(state, session))]
async fn callback(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<OAuthCallbackParams>,
) -> Response {
    // Check for errors from Shopify
    if let Some(error) = &params.error {
        let description = params.error_description.as_deref().unwrap_or_default();
        tracing::error!("Shopify OAuth error: {} - {}", error, description);
        return Redirect::to("/shopify?error=oauth_denied").into_response();
    }

    // Verify HMAC signature from Shopify
    if !verify_shopify_hmac(&params, state.shopify().client_secret()) {
        tracing::error!("Invalid HMAC signature in OAuth callback");
        return Redirect::to("/shopify?error=oauth_invalid_hmac").into_response();
    }

    // Get code and state
    let Some(code) = &params.code else {
        tracing::error!("Missing authorization code in callback");
        return Redirect::to("/shopify?error=oauth_failed").into_response();
    };

    let Some(callback_state) = &params.state else {
        tracing::error!("Missing state parameter in callback");
        return Redirect::to("/shopify?error=oauth_failed").into_response();
    };

    // Verify state matches what we stored
    let stored_state: Option<String> = session.get(OAUTH_STATE_KEY).await.ok().flatten();
    if stored_state.as_ref() != Some(callback_state) {
        tracing::error!("OAuth state mismatch - possible CSRF attack");
        return Redirect::to("/shopify?error=oauth_invalid_state").into_response();
    }

    // Clear the state from session
    let _ = session.remove::<String>(OAUTH_STATE_KEY).await;

    // Exchange code for token
    let token = match state.shopify().exchange_code(code).await {
        Ok(token) => token,
        Err(e) => {
            tracing::error!("Failed to exchange OAuth code: {}", e);
            return Redirect::to("/shopify?error=oauth_exchange_failed").into_response();
        }
    };

    // Store token in database
    let repo = ShopifyTokenRepository::new(state.pool());
    let scopes: Vec<String> = token
        .scope
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    if let Err(e) = repo
        .save(&token.shop, &token.access_token, &scopes, token.obtained_at)
        .await
    {
        tracing::error!("Failed to save Shopify token: {}", e);
        return Redirect::to("/shopify?error=oauth_save_failed").into_response();
    }

    tracing::info!("Successfully connected to Shopify store: {}", token.shop);
    Redirect::to("/shopify?success=connected").into_response()
}

/// GET /shopify/disconnect - Disconnect from Shopify.
#[instrument(skip(state, session))]
async fn disconnect(State(state): State<AppState>, session: Session) -> Response {
    // Check super_admin permission
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let shop = state.shopify().store();

    // Delete token from database
    let repo = ShopifyTokenRepository::new(state.pool());
    if let Err(e) = repo.delete(shop).await {
        tracing::error!("Failed to delete Shopify token: {}", e);
        return Redirect::to("/shopify?error=disconnect_failed").into_response();
    }

    // Clear token from client
    state.shopify().clear_token().await;

    tracing::info!("Disconnected from Shopify store: {}", shop);
    Redirect::to("/shopify?success=disconnected").into_response()
}

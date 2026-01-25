//! Shopify Customer Account OAuth route handlers.
//!
//! Handles the OAuth flow for Shopify Customer Account authentication:
//! - Login: Redirects to Shopify's OAuth authorization page
//! - Callback: Handles the OAuth callback and exchanges code for tokens
//! - Logout: Clears the Shopify customer token and redirects to Shopify logout

use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use rand::Rng;
use serde::Deserialize;
use tower_sessions::Session;

use crate::models::session_keys;
use crate::shopify::CustomerAccessToken;
use crate::state::AppState;

/// Query parameters from Shopify OAuth callback.
#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    /// Authorization code to exchange for tokens.
    pub code: Option<String>,
    /// State parameter for CSRF protection.
    pub state: Option<String>,
    /// Error code if authorization failed.
    pub error: Option<String>,
    /// Error description.
    pub error_description: Option<String>,
}

/// Generate a cryptographically secure random string.
fn generate_random_string(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..length)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            // SAFETY: idx is always within bounds since random_range returns 0..CHARSET.len()
            char::from(*CHARSET.get(idx).expect("idx within bounds"))
        })
        .collect()
}

/// Initiate Shopify Customer Account OAuth login.
///
/// Generates state and nonce parameters, stores them in the session,
/// and redirects to Shopify's authorization page.
///
/// # Route
///
/// `GET /auth/shopify/login`
pub async fn login(State(state): State<AppState>, session: Session) -> Response {
    // Generate CSRF state and OpenID nonce
    let oauth_state = generate_random_string(32);
    let nonce = generate_random_string(32);

    // Store in session for validation on callback
    if let Err(e) = session
        .insert(session_keys::SHOPIFY_OAUTH_STATE, &oauth_state)
        .await
    {
        tracing::error!("Failed to store OAuth state in session: {}", e);
        return Redirect::to("/auth/login?error=session").into_response();
    }

    if let Err(e) = session
        .insert(session_keys::SHOPIFY_OAUTH_NONCE, &nonce)
        .await
    {
        tracing::error!("Failed to store OAuth nonce in session: {}", e);
        return Redirect::to("/auth/login?error=session").into_response();
    }

    // Build the redirect URI
    let redirect_uri = format!("{}/auth/shopify/callback", state.config().base_url);

    // Generate and redirect to authorization URL
    let auth_url = state
        .customer()
        .authorization_url(&redirect_uri, &oauth_state, &nonce);

    Redirect::to(&auth_url).into_response()
}

/// Handle Shopify OAuth callback.
///
/// Validates the state parameter, exchanges the authorization code for tokens,
/// and stores the customer access token in the session.
///
/// # Route
///
/// `GET /auth/shopify/callback`
pub async fn callback(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<CallbackQuery>,
) -> Response {
    // Check for OAuth errors from Shopify
    if let Some(error) = query.error {
        let description = query.error_description.unwrap_or_default();
        tracing::warn!("Shopify OAuth error: {} - {}", error, description);
        return Redirect::to("/auth/login?error=shopify_denied").into_response();
    }

    // Verify we have an authorization code
    let Some(code) = query.code else {
        tracing::warn!("Shopify OAuth callback missing code");
        return Redirect::to("/auth/login?error=missing_code").into_response();
    };

    // Verify state parameter (CSRF protection)
    let Some(returned_state) = query.state else {
        tracing::warn!("Shopify OAuth callback missing state");
        return Redirect::to("/auth/login?error=missing_state").into_response();
    };

    let stored_state: Option<String> = session
        .get(session_keys::SHOPIFY_OAUTH_STATE)
        .await
        .ok()
        .flatten();

    if stored_state.as_ref() != Some(&returned_state) {
        tracing::warn!("Shopify OAuth state mismatch");
        return Redirect::to("/auth/login?error=invalid_state").into_response();
    }

    // Clear the stored state (one-time use)
    let _ = session
        .remove::<String>(session_keys::SHOPIFY_OAUTH_STATE)
        .await;
    let _ = session
        .remove::<String>(session_keys::SHOPIFY_OAUTH_NONCE)
        .await;

    // Build redirect URI (must match the one used in authorization request)
    let redirect_uri = format!("{}/auth/shopify/callback", state.config().base_url);

    // Exchange code for tokens
    let token = match state.customer().exchange_code(&code, &redirect_uri).await {
        Ok(token) => token,
        Err(e) => {
            tracing::error!("Failed to exchange Shopify OAuth code: {}", e);
            return Redirect::to("/auth/login?error=token_exchange").into_response();
        }
    };

    // Store the customer token in the session
    if let Err(e) = session
        .insert(session_keys::SHOPIFY_CUSTOMER_TOKEN, &token)
        .await
    {
        tracing::error!("Failed to store Shopify customer token: {}", e);
        return Redirect::to("/auth/login?error=session").into_response();
    }

    tracing::info!("Shopify customer authenticated successfully");

    // Redirect to account page
    Redirect::to("/account").into_response()
}

/// Logout from Shopify Customer Account.
///
/// Clears the Shopify customer token from the session and optionally
/// redirects to Shopify's logout endpoint.
///
/// # Route
///
/// `POST /auth/shopify/logout`
pub async fn logout(State(state): State<AppState>, session: Session) -> Response {
    // Get the current token to extract id_token for Shopify logout
    let token: Option<CustomerAccessToken> = session
        .get(session_keys::SHOPIFY_CUSTOMER_TOKEN)
        .await
        .ok()
        .flatten();

    // Clear the Shopify customer token from session
    let _ = session
        .remove::<CustomerAccessToken>(session_keys::SHOPIFY_CUSTOMER_TOKEN)
        .await;

    // If we have an id_token, redirect to Shopify logout
    if let Some(token) = token
        && let Some(id_token) = token.id_token
    {
        let post_logout_uri = format!("{}/", state.config().base_url);
        let logout_url = state.customer().logout_url(&id_token, &post_logout_uri);
        return Redirect::to(&logout_url).into_response();
    }

    // Otherwise just redirect to home
    Redirect::to("/").into_response()
}

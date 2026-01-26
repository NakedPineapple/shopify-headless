//! `WebAuthn` API routes.
//!
//! JSON API endpoints for passkey registration and authentication.
//!
//! Passkeys are linked to Shopify customer IDs, allowing customers to authenticate
//! without a password after initial setup.

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use webauthn_rs::prelude::*;

use crate::middleware::{RequireAuth, set_current_customer};
use crate::models::{CurrentCustomer, session_keys};
use crate::services::{AuthError, AuthService};
use crate::state::AppState;

/// Error response for API endpoints.
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
}

impl ApiError {
    fn new(msg: impl Into<String>) -> Self {
        Self { error: msg.into() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(self)).into_response()
    }
}

// ============================================================================
// Registration
// ============================================================================

/// Request to start passkey registration.
#[derive(Debug, Deserialize)]
pub struct StartRegistrationRequest {
    /// Optional name for the passkey (e.g., "MacBook", "iPhone").
    pub name: Option<String>,
}

/// Response from starting passkey registration.
#[derive(Debug, Serialize)]
pub struct StartRegistrationResponse {
    pub options: CreationChallengeResponse,
}

/// Start passkey registration for the current customer.
///
/// POST /api/auth/webauthn/register/start
///
/// The customer must already be logged in (via Shopify password auth).
///
/// # Errors
///
/// Returns `ApiError` if registration fails.
pub async fn start_registration(
    State(state): State<AppState>,
    session: Session,
    RequireAuth(current_customer): RequireAuth,
) -> Result<Json<StartRegistrationResponse>, ApiError> {
    let auth = AuthService::new(state.pool(), state.webauthn());

    // Get existing credentials for this Shopify customer
    let credentials = auth
        .get_credentials_by_shopify_customer_id(&current_customer.shopify_customer_id)
        .await
        .map_err(|e| ApiError::new(e.to_string()))?;

    // Start registration using Shopify customer ID as the user identifier
    let (options, reg_state) = auth
        .start_passkey_registration_for_shopify_customer(
            &current_customer.shopify_customer_id,
            &current_customer.email,
            &credentials,
        )
        .map_err(|e| ApiError::new(e.to_string()))?;

    // Store registration state in session
    session
        .insert(session_keys::WEBAUTHN_REG, reg_state)
        .await
        .map_err(|e| ApiError::new(format!("session error: {e}")))?;

    Ok(Json(StartRegistrationResponse { options }))
}

/// Request to finish passkey registration.
#[derive(Debug, Deserialize)]
pub struct FinishRegistrationRequest {
    /// The `WebAuthn` response from the authenticator.
    pub credential: RegisterPublicKeyCredential,
    /// User-assigned name for this passkey.
    pub name: String,
}

/// Response from finishing passkey registration.
#[derive(Debug, Serialize)]
pub struct FinishRegistrationResponse {
    pub success: bool,
    pub credential_id: i32,
}

/// Finish passkey registration.
///
/// POST /api/auth/webauthn/register/finish
///
/// # Errors
///
/// Returns `ApiError` if registration fails.
pub async fn finish_registration(
    State(state): State<AppState>,
    session: Session,
    RequireAuth(current_customer): RequireAuth,
    Json(req): Json<FinishRegistrationRequest>,
) -> Result<Json<FinishRegistrationResponse>, ApiError> {
    // Get registration state from session
    let reg_state: PasskeyRegistration = session
        .get(session_keys::WEBAUTHN_REG)
        .await
        .map_err(|e| ApiError::new(format!("session error: {e}")))?
        .ok_or_else(|| ApiError::new("no registration in progress"))?;

    // Clear registration state
    let _ = session
        .remove::<PasskeyRegistration>(session_keys::WEBAUTHN_REG)
        .await;

    let auth = AuthService::new(state.pool(), state.webauthn());

    // Finish registration
    let passkey = auth
        .finish_passkey_registration(&reg_state, &req.credential)
        .map_err(|e| ApiError::new(e.to_string()))?;

    // Parse email for credential storage (enables passkey-by-email lookup)
    let email = current_customer
        .email_parsed()
        .map_err(|e| ApiError::new(format!("invalid email: {e}")))?;

    // Save credential linked to Shopify customer ID and email
    let credential = auth
        .save_credential_for_shopify_customer(
            &current_customer.shopify_customer_id,
            &email,
            &passkey,
            &req.name,
        )
        .await
        .map_err(|e| ApiError::new(e.to_string()))?;

    Ok(Json(FinishRegistrationResponse {
        success: true,
        credential_id: credential.id.as_i32(),
    }))
}

// ============================================================================
// Authentication
// ============================================================================

/// Request to start passkey authentication.
#[derive(Debug, Deserialize)]
pub struct StartAuthenticationRequest {
    pub email: String,
}

/// Response from starting passkey authentication.
#[derive(Debug, Serialize)]
pub struct StartAuthenticationResponse {
    pub options: RequestChallengeResponse,
}

/// Start passkey authentication.
///
/// POST /api/auth/webauthn/authenticate/start
///
/// # Errors
///
/// Returns `ApiError` if authentication fails.
pub async fn start_authentication(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<StartAuthenticationRequest>,
) -> Result<Json<StartAuthenticationResponse>, ApiError> {
    let auth = AuthService::new(state.pool(), state.webauthn());

    // Start authentication - this looks up credentials by email
    // and returns the Shopify customer ID for verification after auth
    let (options, auth_state, shopify_customer_id) = auth
        .start_passkey_authentication_for_shopify_customer(&req.email)
        .await
        .map_err(|e| match e {
            AuthError::UserNotFound => ApiError::new("no account found with this email"),
            AuthError::NoCredentials => ApiError::new("no passkeys registered for this account"),
            other => ApiError::new(other.to_string()),
        })?;

    // Store authentication state in session (includes Shopify customer ID for verification)
    session
        .insert(
            session_keys::WEBAUTHN_AUTH,
            (auth_state, shopify_customer_id),
        )
        .await
        .map_err(|e| ApiError::new(format!("session error: {e}")))?;

    Ok(Json(StartAuthenticationResponse { options }))
}

/// Request to finish passkey authentication.
#[derive(Debug, Deserialize)]
pub struct FinishAuthenticationRequest {
    pub credential: PublicKeyCredential,
}

/// Response from finishing passkey authentication.
#[derive(Debug, Serialize)]
pub struct FinishAuthenticationResponse {
    pub success: bool,
    pub redirect: String,
}

/// Finish passkey authentication.
///
/// POST /api/auth/webauthn/authenticate/finish
///
/// # Errors
///
/// Returns `ApiError` if authentication fails.
pub async fn finish_authentication(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<FinishAuthenticationRequest>,
) -> Result<Json<FinishAuthenticationResponse>, ApiError> {
    // Get authentication state from session
    let (auth_state, shopify_customer_id): (PasskeyAuthentication, String) = session
        .get(session_keys::WEBAUTHN_AUTH)
        .await
        .map_err(|e| ApiError::new(format!("session error: {e}")))?
        .ok_or_else(|| ApiError::new("no authentication in progress"))?;

    // Clear authentication state
    let _ = session
        .remove::<(PasskeyAuthentication, String)>(session_keys::WEBAUTHN_AUTH)
        .await;

    let auth = AuthService::new(state.pool(), state.webauthn());

    // Finish authentication - verifies the passkey response
    auth.finish_passkey_authentication_for_shopify_customer(
        &auth_state,
        &req.credential,
        &shopify_customer_id,
    )
    .await
    .map_err(|e| ApiError::new(e.to_string()))?;

    // After successful passkey auth, we need to get customer data from Shopify
    // and create an access token. For now, we'll create a session with the customer ID
    // but without a Shopify access token (the customer will need to login with password
    // to get a full session with Shopify API access).
    //
    // TODO: Consider using Shopify's customerAccessTokenCreateWithMultipass for
    // full Shopify integration, or store a long-lived token during password auth.

    // For now, fetch customer info from Shopify to populate the session
    // This requires that the customer already has a stored access token or we skip this
    // In a production implementation, you might want to:
    // 1. Store a refresh token during password auth
    // 2. Use multipass for seamless token creation
    // 3. Require password auth periodically to refresh tokens

    // Create a minimal session - the customer is authenticated via passkey
    // but doesn't have a fresh Shopify access token
    let current_customer = CurrentCustomer::new(
        shopify_customer_id,
        String::new(), // Email will be fetched when needed
        None,
        None,
        SecretString::from(String::new()), // No access token for passkey-only auth
        String::new(),                     // No expiry
    );

    set_current_customer(&session, &current_customer)
        .await
        .map_err(|e| ApiError::new(format!("session error: {e}")))?;

    Ok(Json(FinishAuthenticationResponse {
        success: true,
        redirect: "/account".to_owned(),
    }))
}

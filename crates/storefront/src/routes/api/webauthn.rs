//! `WebAuthn` API routes.
//!
//! JSON API endpoints for passkey registration and authentication.

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use webauthn_rs::prelude::*;

use crate::middleware::{RequireAuth, set_current_user};
use crate::models::{CurrentUser, session_keys};
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

/// Start passkey registration for the current user.
///
/// POST /api/auth/webauthn/register/start
///
/// # Errors
///
/// Returns `ApiError` if registration fails.
pub async fn start_registration(
    State(state): State<AppState>,
    session: Session,
    RequireAuth(current_user): RequireAuth,
) -> Result<Json<StartRegistrationResponse>, ApiError> {
    let auth = AuthService::new(state.pool(), state.webauthn());

    // Get user and existing credentials
    let user = auth
        .get_user(current_user.id)
        .await
        .map_err(|e| ApiError::new(e.to_string()))?;

    let credentials = auth
        .get_credentials(current_user.id)
        .await
        .map_err(|e| ApiError::new(e.to_string()))?;

    // Start registration
    let (options, reg_state) = auth
        .start_passkey_registration(&user, &credentials)
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
    RequireAuth(current_user): RequireAuth,
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

    // Save credential
    let credential = auth
        .save_credential(current_user.id, &passkey, &req.name)
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

    // Start authentication
    let (options, auth_state, user_id) = auth
        .start_passkey_authentication(&req.email)
        .await
        .map_err(|e| match e {
            AuthError::UserNotFound => ApiError::new("user not found"),
            AuthError::NoCredentials => ApiError::new("no passkeys registered"),
            other => ApiError::new(other.to_string()),
        })?;

    // Store authentication state in session (includes user_id for verification)
    session
        .insert(session_keys::WEBAUTHN_AUTH, (auth_state, user_id))
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
    use naked_pineapple_core::UserId;

    // Get authentication state from session
    let (auth_state, user_id): (PasskeyAuthentication, UserId) = session
        .get(session_keys::WEBAUTHN_AUTH)
        .await
        .map_err(|e| ApiError::new(format!("session error: {e}")))?
        .ok_or_else(|| ApiError::new("no authentication in progress"))?;

    // Clear authentication state
    let _ = session
        .remove::<(PasskeyAuthentication, UserId)>(session_keys::WEBAUTHN_AUTH)
        .await;

    let auth = AuthService::new(state.pool(), state.webauthn());

    // Finish authentication
    let user = auth
        .finish_passkey_authentication(&auth_state, &req.credential, user_id)
        .await
        .map_err(|e| ApiError::new(e.to_string()))?;

    // Set current user in session
    let current_user = CurrentUser {
        id: user.id,
        email: user.email,
    };

    set_current_user(&session, &current_user)
        .await
        .map_err(|e| ApiError::new(format!("session error: {e}")))?;

    Ok(Json(FinishAuthenticationResponse {
        success: true,
        redirect: "/account".to_owned(),
    }))
}

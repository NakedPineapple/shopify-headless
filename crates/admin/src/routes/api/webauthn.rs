//! `WebAuthn` API routes for admin authentication.
//!
//! JSON API endpoints for passkey registration and authentication.

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use webauthn_rs::prelude::*;

use crate::middleware::{RequireAdminAuth, set_current_admin};
use crate::models::{CurrentAdmin, session_keys};
use crate::services::{AdminAuthError, AdminAuthService};
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

/// Build the `WebAuthn` API router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/auth/webauthn/authenticate/start",
            post(start_authentication),
        )
        .route(
            "/api/auth/webauthn/authenticate/finish",
            post(finish_authentication),
        )
        .route(
            "/api/auth/webauthn/register/start",
            post(start_registration),
        )
        .route(
            "/api/auth/webauthn/register/finish",
            post(finish_registration),
        )
}

// ============================================================================
// Registration (requires authentication)
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

/// Start passkey registration for the current admin.
///
/// POST /api/auth/webauthn/register/start
///
/// Requires authentication.
///
/// # Errors
///
/// Returns `ApiError` if registration fails.
pub async fn start_registration(
    State(state): State<AppState>,
    session: Session,
    RequireAdminAuth(current_admin): RequireAdminAuth,
) -> Result<Json<StartRegistrationResponse>, ApiError> {
    let auth = AdminAuthService::new(state.pool(), state.webauthn());

    // Get user and existing credentials
    let user = auth
        .get_user(current_admin.id)
        .await
        .map_err(|e| ApiError::new(e.to_string()))?;

    let credentials = auth
        .get_credentials(current_admin.id)
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
/// Requires authentication.
///
/// # Errors
///
/// Returns `ApiError` if registration fails.
pub async fn finish_registration(
    State(state): State<AppState>,
    session: Session,
    RequireAdminAuth(current_admin): RequireAdminAuth,
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

    let auth = AdminAuthService::new(state.pool(), state.webauthn());

    // Finish registration
    let passkey = auth
        .finish_passkey_registration(&reg_state, &req.credential)
        .map_err(|e| ApiError::new(e.to_string()))?;

    // Save credential
    let credential = auth
        .save_credential(current_admin.id, &passkey, &req.name)
        .await
        .map_err(|e| ApiError::new(e.to_string()))?;

    Ok(Json(FinishRegistrationResponse {
        success: true,
        credential_id: credential.id.as_i32(),
    }))
}

// ============================================================================
// Authentication (no auth required) - Discoverable Credentials
// ============================================================================

/// Response from starting passkey authentication.
#[derive(Debug, Serialize)]
pub struct StartAuthenticationResponse {
    pub options: RequestChallengeResponse,
}

/// Start discoverable passkey authentication.
///
/// POST /api/auth/webauthn/authenticate/start
///
/// No email required - authenticator will present available credentials.
///
/// # Errors
///
/// Returns `ApiError` if authentication fails.
pub async fn start_authentication(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<StartAuthenticationResponse>, ApiError> {
    let auth = AdminAuthService::new(state.pool(), state.webauthn());

    // Start discoverable authentication - no email needed
    let (options, auth_state) = auth
        .start_passkey_authentication()
        .await
        .map_err(|e| match e {
            AdminAuthError::NoCredentials => ApiError::new("no admin accounts exist"),
            other => ApiError::new(other.to_string()),
        })?;

    // Store authentication state in session
    session
        .insert(session_keys::WEBAUTHN_AUTH, auth_state)
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

/// Finish discoverable passkey authentication.
///
/// POST /api/auth/webauthn/authenticate/finish
///
/// The user is identified by the user handle stored in the passkey.
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
    let auth_state: DiscoverableAuthentication = session
        .get(session_keys::WEBAUTHN_AUTH)
        .await
        .map_err(|e| ApiError::new(format!("session error: {e}")))?
        .ok_or_else(|| ApiError::new("no authentication in progress"))?;

    // Clear authentication state
    let _ = session
        .remove::<DiscoverableAuthentication>(session_keys::WEBAUTHN_AUTH)
        .await;

    let auth = AdminAuthService::new(state.pool(), state.webauthn());

    // Finish authentication - user is identified by the user handle in the credential
    let user = auth
        .finish_passkey_authentication(&auth_state, &req.credential)
        .await
        .map_err(|e| ApiError::new(e.to_string()))?;

    // Set current admin in session
    let current_admin = CurrentAdmin {
        id: user.id,
        email: user.email,
        name: user.name,
        role: user.role,
    };

    set_current_admin(&session, &current_admin)
        .await
        .map_err(|e| ApiError::new(format!("session error: {e}")))?;

    Ok(Json(FinishAuthenticationResponse {
        success: true,
        redirect: "/chat".to_owned(),
    }))
}

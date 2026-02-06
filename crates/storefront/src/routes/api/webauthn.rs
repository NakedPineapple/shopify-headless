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
use webauthn_rs::prelude::{
    CreationChallengeResponse, DiscoverableAuthentication, PasskeyRegistration,
    PublicKeyCredential, RegisterPublicKeyCredential, RequestChallengeResponse,
};

use crate::error::set_sentry_user;
use crate::middleware::{RequireAuth, set_current_customer};
use crate::models::{CurrentCustomer, session_keys};
use crate::services::AuthService;
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
    site: crate::middleware::SiteContext,
    RequireAuth(current_customer): RequireAuth,
) -> Result<Json<StartRegistrationResponse>, ApiError> {
    let auth = AuthService::new(state.pool(), state.webauthn_for_host(&site.host));

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
    site: crate::middleware::SiteContext,
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

    let auth = AuthService::new(state.pool(), state.webauthn_for_host(&site.host));

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
// Authentication (Discoverable Credentials)
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
/// No email is required — the browser presents all saved passkeys for this site.
///
/// # Errors
///
/// Returns `ApiError` if authentication fails.
pub async fn start_authentication(
    State(state): State<AppState>,
    session: Session,
    site: crate::middleware::SiteContext,
) -> Result<Json<StartAuthenticationResponse>, ApiError> {
    let auth = AuthService::new(state.pool(), state.webauthn_for_host(&site.host));

    let (options, auth_state) = auth
        .start_discoverable_authentication_for_shopify_customer()
        .map_err(|e| ApiError::new(e.to_string()))?;

    // Store discoverable authentication state in session
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
/// # Errors
///
/// Returns `ApiError` if authentication fails.
pub async fn finish_authentication(
    State(state): State<AppState>,
    session: Session,
    site: crate::middleware::SiteContext,
    Json(req): Json<FinishAuthenticationRequest>,
) -> Result<Json<FinishAuthenticationResponse>, ApiError> {
    // Get discoverable authentication state from session
    let auth_state: DiscoverableAuthentication = session
        .get(session_keys::WEBAUTHN_AUTH)
        .await
        .map_err(|e| ApiError::new(format!("session error: {e}")))?
        .ok_or_else(|| ApiError::new("no authentication in progress"))?;

    // Clear authentication state
    let _ = session
        .remove::<DiscoverableAuthentication>(session_keys::WEBAUTHN_AUTH)
        .await;

    let auth = AuthService::new(state.pool(), state.webauthn_for_host(&site.host));

    // Finish authentication - looks up credential by WebAuthn ID and verifies
    let credential = auth
        .finish_discoverable_authentication_for_shopify_customer(auth_state, &req.credential)
        .await
        .map_err(|e| ApiError::new(e.to_string()))?;

    let email = credential.email.map(|e| e.to_string()).unwrap_or_default();

    // Create a minimal session - the customer is authenticated via passkey
    // but doesn't have a fresh Shopify access token
    //
    // TODO: Consider using Shopify's customerAccessTokenCreateWithMultipass for
    // full Shopify integration, or store a long-lived token during password auth.
    let current_customer = CurrentCustomer::new(
        credential.shopify_customer_id,
        email,
        None,
        None,
        SecretString::from(String::new()), // No access token for passkey-only auth
        String::new(),                     // No expiry
    );

    set_current_customer(&session, &current_customer)
        .await
        .map_err(|e| ApiError::new(format!("session error: {e}")))?;

    set_sentry_user(
        &current_customer.shopify_customer_id,
        Some(&current_customer.email),
    );

    Ok(Json(FinishAuthenticationResponse {
        success: true,
        redirect: "/account".to_owned(),
    }))
}

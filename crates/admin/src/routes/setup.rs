//! Admin setup routes for new user registration.
//!
//! Provides a 3-step email-verified passkey registration flow:
//! 1. Enter email → send verification code
//! 2. Enter code → verify email
//! 3. Create passkey → complete registration

use askama::Template;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use webauthn_rs::prelude::*;

use naked_pineapple_core::Email;

use crate::db::{AdminInviteRepository, AdminUserRepository};
use crate::filters;
use crate::middleware::set_current_admin;
use crate::models::{CurrentAdmin, session_keys};
use crate::services::{AdminAuthService, EmailService, generate_verification_code};
use crate::state::AppState;

/// Session keys for setup flow.
mod setup_session_keys {
    pub const VERIFICATION_CODE: &str = "setup_verification_code";
    pub const VERIFICATION_EMAIL: &str = "setup_verification_email";
    pub const VERIFICATION_EXPIRES: &str = "setup_verification_expires";
    pub const EMAIL_VERIFIED: &str = "setup_email_verified";
}

/// Pending registration info stored in session.
#[derive(Serialize, Deserialize)]
struct PendingRegistration {
    email: String,
    display_name: String,
    passkey_name: String,
}

/// Setup page template.
#[derive(Template)]
#[template(path = "auth/setup.html")]
struct SetupPageTemplate;

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

/// Build the setup router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/setup", get(setup_page))
        // Email verification
        .route("/api/auth/setup/send-code", post(send_verification_code))
        .route("/api/auth/setup/verify-code", post(verify_code))
        // Passkey registration (for setup - no auth required, but email must be verified)
        .route("/api/auth/setup/register/start", post(register_start))
        .route("/api/auth/setup/register/finish", post(register_finish))
}

/// Render the setup page.
///
/// GET /auth/setup
async fn setup_page() -> impl IntoResponse {
    Html(
        SetupPageTemplate
            .render()
            .unwrap_or_else(|_| String::from("Error rendering template")),
    )
}

// =============================================================================
// Step 1: Send Verification Code
// =============================================================================

/// Request to send a verification code.
#[derive(Debug, Deserialize)]
pub struct SendCodeRequest {
    pub email: String,
}

/// Response after sending verification code.
#[derive(Debug, Serialize)]
pub struct SendCodeResponse {
    pub success: bool,
    pub message: String,
}

/// Send a verification code to the email if it has a valid invite.
///
/// POST /api/auth/setup/send-code
async fn send_verification_code(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<SendCodeRequest>,
) -> Result<Json<SendCodeResponse>, ApiError> {
    let email = req.email.trim().to_lowercase();

    // Validate email format
    let parsed_email = Email::parse(&email).map_err(|_| ApiError::new("Invalid email address"))?;

    // Check if invite exists and is valid
    let invite_repo = AdminInviteRepository::new(state.pool());
    let is_valid = invite_repo
        .is_valid_invite(&email)
        .await
        .map_err(|e| ApiError::new(format!("Database error: {e}")))?;

    if !is_valid {
        return Err(ApiError::new(
            "No valid invite found for this email address",
        ));
    }

    // Check if an admin already exists with this email
    let user_repo = AdminUserRepository::new(state.pool());
    let existing = user_repo
        .get_by_email(&parsed_email)
        .await
        .map_err(|e| ApiError::new(format!("Database error: {e}")))?;

    if existing.is_some() {
        return Err(ApiError::new(
            "An admin account already exists with this email",
        ));
    }

    // Generate verification code
    let code = generate_verification_code();
    let expires_at = Utc::now() + chrono::Duration::minutes(10);

    // Store in session
    session
        .insert(setup_session_keys::VERIFICATION_CODE, &code)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?;
    session
        .insert(setup_session_keys::VERIFICATION_EMAIL, &email)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?;
    session
        .insert(
            setup_session_keys::VERIFICATION_EXPIRES,
            expires_at.timestamp(),
        )
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?;

    // Clear any previous verification
    let _ = session
        .remove::<bool>(setup_session_keys::EMAIL_VERIFIED)
        .await;

    // Send email
    if let Some(email_service) = state.email_service() {
        email_service
            .send_verification_code(&email, &code)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to send verification email");
                ApiError::new("Failed to send verification email. Please try again.")
            })?;
    } else {
        // Development mode - log the code
        tracing::warn!(
            email = %email,
            code = %code,
            "SMTP not configured - verification code logged (dev mode)"
        );
    }

    Ok(Json(SendCodeResponse {
        success: true,
        message: "Verification code sent to your email".to_owned(),
    }))
}

// =============================================================================
// Step 2: Verify Code
// =============================================================================

/// Request to verify a code.
#[derive(Debug, Deserialize)]
pub struct VerifyCodeRequest {
    pub code: String,
}

/// Response after verifying code.
#[derive(Debug, Serialize)]
pub struct VerifyCodeResponse {
    pub success: bool,
    pub email: String,
    pub name: String,
}

/// Verify the code entered by the user.
///
/// POST /api/auth/setup/verify-code
async fn verify_code(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<VerifyCodeRequest>,
) -> Result<Json<VerifyCodeResponse>, ApiError> {
    // Get stored verification data
    let stored_code: String = session
        .get(setup_session_keys::VERIFICATION_CODE)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?
        .ok_or_else(|| ApiError::new("No verification in progress. Please request a new code."))?;

    let stored_email: String = session
        .get(setup_session_keys::VERIFICATION_EMAIL)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?
        .ok_or_else(|| ApiError::new("No verification in progress. Please request a new code."))?;

    let expires_timestamp: i64 = session
        .get(setup_session_keys::VERIFICATION_EXPIRES)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?
        .ok_or_else(|| ApiError::new("No verification in progress. Please request a new code."))?;

    // Check expiration
    let now = Utc::now().timestamp();
    if now > expires_timestamp {
        // Clear expired verification
        let _ = session
            .remove::<String>(setup_session_keys::VERIFICATION_CODE)
            .await;
        return Err(ApiError::new(
            "Verification code has expired. Please request a new code.",
        ));
    }

    // Verify code
    if req.code.trim() != stored_code {
        return Err(ApiError::new("Invalid verification code"));
    }

    // Clear the code (one-time use)
    let _ = session
        .remove::<String>(setup_session_keys::VERIFICATION_CODE)
        .await;

    // Mark email as verified
    session
        .insert(setup_session_keys::EMAIL_VERIFIED, true)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?;

    // Get invite details
    let invite_repo = AdminInviteRepository::new(state.pool());
    let invite = invite_repo
        .get_by_email(&stored_email)
        .await
        .map_err(|e| ApiError::new(format!("Database error: {e}")))?
        .ok_or_else(|| ApiError::new("Invite not found"))?;

    Ok(Json(VerifyCodeResponse {
        success: true,
        email: stored_email,
        name: invite.name,
    }))
}

// =============================================================================
// Step 3: Passkey Registration
// =============================================================================

/// Request to start passkey registration.
#[derive(Debug, Deserialize)]
pub struct StartRegistrationRequest {
    pub display_name: String,
    pub passkey_name: Option<String>,
}

/// Response from starting passkey registration.
#[derive(Debug, Serialize)]
pub struct StartRegistrationResponse {
    pub options: CreationChallengeResponse,
}

/// Start passkey registration for the verified email.
///
/// POST /api/auth/setup/register/start
async fn register_start(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<StartRegistrationRequest>,
) -> Result<Json<StartRegistrationResponse>, ApiError> {
    // Verify email was verified
    let email_verified: bool = session
        .get(setup_session_keys::EMAIL_VERIFIED)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?
        .unwrap_or(false);

    if !email_verified {
        return Err(ApiError::new(
            "Email not verified. Please verify your email first.",
        ));
    }

    let email: String = session
        .get(setup_session_keys::VERIFICATION_EMAIL)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?
        .ok_or_else(|| ApiError::new("No email in session. Please start over."))?;

    // Validate invite is still valid
    let invite_repo = AdminInviteRepository::new(state.pool());
    let invite = invite_repo
        .get_by_email(&email)
        .await
        .map_err(|e| ApiError::new(format!("Database error: {e}")))?
        .ok_or_else(|| ApiError::new("Invite not found"))?;

    if !invite.is_valid() {
        return Err(ApiError::new("Invite is no longer valid"));
    }

    // Generate a temporary user ID for WebAuthn (will be replaced with real ID after creation)
    let temp_user_id = uuid::Uuid::new_v4();

    // Start passkey registration with WebAuthn
    let (ccr, reg_state) = state
        .webauthn()
        .start_passkey_registration(
            temp_user_id,
            &email,
            &req.display_name,
            None, // No existing credentials
        )
        .map_err(|e| ApiError::new(format!("WebAuthn error: {e}")))?;

    // Store registration state and pending info
    session
        .insert(session_keys::WEBAUTHN_REG, reg_state)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?;

    // Store pending registration info
    let pending = PendingRegistration {
        email,
        display_name: req.display_name,
        passkey_name: req.passkey_name.unwrap_or_else(|| "Passkey".to_owned()),
    };

    session
        .insert("setup_pending_registration", pending)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?;

    Ok(Json(StartRegistrationResponse { options: ccr }))
}

/// Request to finish passkey registration.
#[derive(Debug, Deserialize)]
pub struct FinishRegistrationRequest {
    pub credential: RegisterPublicKeyCredential,
}

/// Response from finishing passkey registration.
#[derive(Debug, Serialize)]
pub struct FinishRegistrationResponse {
    pub success: bool,
    pub redirect: String,
}

/// Finish passkey registration and create admin user.
///
/// POST /api/auth/setup/register/finish
async fn register_finish(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<FinishRegistrationRequest>,
) -> Result<Json<FinishRegistrationResponse>, ApiError> {
    // Get registration state
    let reg_state: PasskeyRegistration = session
        .get(session_keys::WEBAUTHN_REG)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?
        .ok_or_else(|| ApiError::new("No registration in progress"))?;

    // Get pending registration info
    let pending: PendingRegistration = session
        .get("setup_pending_registration")
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?
        .ok_or_else(|| ApiError::new("No registration in progress"))?;

    // Clear registration state
    let _ = session
        .remove::<PasskeyRegistration>(session_keys::WEBAUTHN_REG)
        .await;
    let _ = session
        .remove::<PendingRegistration>("setup_pending_registration")
        .await;

    // Finish WebAuthn registration
    let passkey = state
        .webauthn()
        .finish_passkey_registration(&req.credential, &reg_state)
        .map_err(|e| ApiError::new(format!("WebAuthn error: {e}")))?;

    // Get invite to get role
    let invite_repo = AdminInviteRepository::new(state.pool());
    let invite = invite_repo
        .get_by_email(&pending.email)
        .await
        .map_err(|e| ApiError::new(format!("Database error: {e}")))?
        .ok_or_else(|| ApiError::new("Invite not found"))?;

    if !invite.is_valid() {
        return Err(ApiError::new("Invite is no longer valid"));
    }

    // Parse email
    let email = Email::parse(&pending.email).map_err(|_| ApiError::new("Invalid email address"))?;

    // Create admin user
    let user_repo = AdminUserRepository::new(state.pool());
    let user = user_repo
        .create(&email, &pending.display_name, invite.role)
        .await
        .map_err(|e| ApiError::new(format!("Failed to create user: {e}")))?;

    // Create credential
    user_repo
        .create_credential(user.id, &passkey, &pending.passkey_name)
        .await
        .map_err(|e| ApiError::new(format!("Failed to save credential: {e}")))?;

    // Mark invite as used
    invite_repo
        .mark_used(&pending.email, user.id)
        .await
        .map_err(|e| ApiError::new(format!("Failed to update invite: {e}")))?;

    // Clear setup session data
    let _ = session
        .remove::<bool>(setup_session_keys::EMAIL_VERIFIED)
        .await;
    let _ = session
        .remove::<String>(setup_session_keys::VERIFICATION_EMAIL)
        .await;

    // Log the user in
    let current_admin = CurrentAdmin {
        id: user.id,
        email: user.email,
        name: user.name,
        role: user.role,
    };

    set_current_admin(&session, &current_admin)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?;

    tracing::info!(
        admin_id = %user.id.as_i32(),
        email = %pending.email,
        "New admin user registered successfully"
    );

    // Send welcome email
    if let Some(email_service) = state.email_service()
        && let Err(e) = email_service
            .send_welcome_email(&pending.email, &pending.display_name)
            .await
    {
        tracing::warn!(error = %e, "Failed to send welcome email");
    }

    Ok(Json(FinishRegistrationResponse {
        success: true,
        redirect: "/chat".to_owned(),
    }))
}

//! Admin settings routes.
//!
//! Provides profile management and passkey settings for admin users.

use askama::Template;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{delete, get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use tracing::instrument;

use naked_pineapple_core::{AdminCredentialId, Email};

use crate::db::AdminUserRepository;
use crate::filters;
use crate::middleware::{RequireAdminAuth, set_current_admin};
use crate::models::CurrentAdmin;
use crate::services::{AdminAuthService, EmailService, generate_verification_code};
use crate::state::AppState;

use super::dashboard::AdminUserView;

/// Session keys for email change verification.
mod email_change_keys {
    pub const CODE: &str = "settings_email_change_code";
    pub const TARGET: &str = "settings_email_change_target";
    pub const EXPIRES: &str = "settings_email_change_expires";
}

// =============================================================================
// Templates
// =============================================================================

/// Passkey view for template rendering.
#[derive(Debug, Clone)]
pub struct PasskeyView {
    pub id: i32,
    pub name: String,
    pub created_at: String,
    pub is_only_passkey: bool,
}

/// Settings page template.
#[derive(Template)]
#[template(path = "settings/index.html")]
pub struct SettingsTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub passkeys: Vec<PasskeyView>,
    pub success_message: Option<String>,
    pub error_message: Option<String>,
}

/// Build the settings router.
pub fn router() -> Router<AppState> {
    Router::new()
        // Page
        .route("/settings", get(settings_page))
        // Profile API
        .route("/api/settings/profile", post(update_profile))
        // Email change API
        .route("/api/settings/email/send-code", post(send_email_code))
        .route("/api/settings/email/verify", post(verify_email))
        // Passkey API
        .route("/api/settings/passkeys/{id}", delete(delete_passkey))
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

/// Request to update profile.
#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub name: String,
}

/// Response after updating profile.
#[derive(Debug, Serialize)]
pub struct UpdateProfileResponse {
    pub success: bool,
    pub name: String,
}

/// Request to send email verification code.
#[derive(Debug, Deserialize)]
pub struct SendEmailCodeRequest {
    pub email: String,
}

/// Response after sending email code.
#[derive(Debug, Serialize)]
pub struct SendEmailCodeResponse {
    pub success: bool,
    pub message: String,
}

/// Request to verify email code.
#[derive(Debug, Deserialize)]
pub struct VerifyEmailRequest {
    pub code: String,
}

/// Response after verifying email.
#[derive(Debug, Serialize)]
pub struct VerifyEmailResponse {
    pub success: bool,
    pub email: String,
}

/// Response after deleting passkey.
#[derive(Debug, Serialize)]
pub struct DeletePasskeyResponse {
    pub success: bool,
}

// =============================================================================
// Settings Page
// =============================================================================

/// Render the settings page.
///
/// GET /settings
#[instrument(skip(state))]
async fn settings_page(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Query(params): Query<SettingsQueryParams>,
) -> Response {
    // Get credentials for this admin
    let auth = AdminAuthService::new(state.pool(), state.webauthn());
    let credentials = match auth.get_credentials(admin.id).await {
        Ok(creds) => creds,
        Err(e) => {
            tracing::error!(error = %e, "Failed to get credentials");
            return Redirect::to("/").into_response();
        }
    };

    let credential_count = credentials.len();
    let passkeys: Vec<PasskeyView> = credentials
        .into_iter()
        .map(|c| PasskeyView {
            id: c.id.as_i32(),
            name: c.name,
            created_at: c.created_at.format("%b %d, %Y").to_string(),
            is_only_passkey: credential_count == 1,
        })
        .collect();

    // Map success/error messages
    let success_message = params.success.map(|s| match s.as_str() {
        "profile_updated" => "Your profile has been updated.".to_owned(),
        "email_changed" => "Your email address has been changed.".to_owned(),
        "passkey_deleted" => "Passkey deleted successfully.".to_owned(),
        "passkey_added" => "New passkey added successfully.".to_owned(),
        _ => s,
    });

    let error_message = params.error.map(|e| match e.as_str() {
        "last_passkey" => "Cannot delete your only passkey.".to_owned(),
        "email_taken" => "That email address is already in use.".to_owned(),
        _ => e,
    });

    let template = SettingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/settings".to_owned(),
        passkeys,
        success_message,
        error_message,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

// =============================================================================
// Profile API
// =============================================================================

/// Update the admin's display name.
///
/// POST /api/settings/profile
#[instrument(skip(state, session))]
async fn update_profile(
    State(state): State<AppState>,
    session: Session,
    RequireAdminAuth(admin): RequireAdminAuth,
    Json(req): Json<UpdateProfileRequest>,
) -> Result<Json<UpdateProfileResponse>, ApiError> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(ApiError::new("Name cannot be empty"));
    }

    if name.len() > 100 {
        return Err(ApiError::new("Name is too long"));
    }

    // Update in database
    let repo = AdminUserRepository::new(state.pool());
    let updated_user = repo
        .update_name(admin.id, name)
        .await
        .map_err(|e| ApiError::new(format!("Failed to update profile: {e}")))?;

    // Update session
    let current_admin = CurrentAdmin {
        id: updated_user.id,
        email: updated_user.email,
        name: updated_user.name.clone(),
        role: updated_user.role,
    };
    set_current_admin(&session, &current_admin)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?;

    Ok(Json(UpdateProfileResponse {
        success: true,
        name: updated_user.name,
    }))
}

// =============================================================================
// Email Change API
// =============================================================================

/// Send a verification code to the new email address.
///
/// POST /api/settings/email/send-code
#[instrument(skip(state, session))]
async fn send_email_code(
    State(state): State<AppState>,
    session: Session,
    RequireAdminAuth(admin): RequireAdminAuth,
    Json(req): Json<SendEmailCodeRequest>,
) -> Result<Json<SendEmailCodeResponse>, ApiError> {
    let new_email = req.email.trim().to_lowercase();

    // Validate email format
    let parsed_email =
        Email::parse(&new_email).map_err(|_| ApiError::new("Invalid email address"))?;

    // Check it's different from current email
    if parsed_email == admin.email {
        return Err(ApiError::new("This is already your email address"));
    }

    // Check email isn't taken by another admin
    let repo = AdminUserRepository::new(state.pool());
    if let Some(_existing) = repo
        .get_by_email(&parsed_email)
        .await
        .map_err(|e| ApiError::new(format!("Database error: {e}")))?
    {
        return Err(ApiError::new("This email address is already in use"));
    }

    // Generate verification code
    let code = generate_verification_code();
    let expires_at = Utc::now() + chrono::Duration::minutes(10);

    // Store in session
    session
        .insert(email_change_keys::CODE, &code)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?;
    session
        .insert(email_change_keys::TARGET, &new_email)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?;
    session
        .insert(email_change_keys::EXPIRES, expires_at.timestamp())
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?;

    // Send verification email
    if let Some(email_service) = state.email_service() {
        email_service
            .send_verification_code(&new_email, &code)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to send verification email");
                ApiError::new("Failed to send verification email. Please try again.")
            })?;
    } else {
        // Development mode - log the code
        tracing::warn!(
            email = %new_email,
            code = %code,
            "SMTP not configured - verification code logged (dev mode)"
        );
    }

    Ok(Json(SendEmailCodeResponse {
        success: true,
        message: "Verification code sent to your new email address".to_owned(),
    }))
}

/// Verify the code and complete email change.
///
/// POST /api/settings/email/verify
#[instrument(skip(state, session))]
async fn verify_email(
    State(state): State<AppState>,
    session: Session,
    RequireAdminAuth(admin): RequireAdminAuth,
    Json(req): Json<VerifyEmailRequest>,
) -> Result<Json<VerifyEmailResponse>, ApiError> {
    // Get stored verification data
    let stored_code: String = session
        .get(email_change_keys::CODE)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?
        .ok_or_else(|| ApiError::new("No email change in progress. Please request a new code."))?;

    let target_email: String = session
        .get(email_change_keys::TARGET)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?
        .ok_or_else(|| ApiError::new("No email change in progress. Please request a new code."))?;

    let expires_timestamp: i64 = session
        .get(email_change_keys::EXPIRES)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?
        .ok_or_else(|| ApiError::new("No email change in progress. Please request a new code."))?;

    // Check expiration
    let now = Utc::now().timestamp();
    if now > expires_timestamp {
        // Clear expired verification
        let _ = session.remove::<String>(email_change_keys::CODE).await;
        let _ = session.remove::<String>(email_change_keys::TARGET).await;
        let _ = session.remove::<i64>(email_change_keys::EXPIRES).await;
        return Err(ApiError::new(
            "Verification code has expired. Please request a new code.",
        ));
    }

    // Verify code
    if req.code.trim() != stored_code {
        return Err(ApiError::new("Invalid verification code"));
    }

    // Clear verification state
    let _ = session.remove::<String>(email_change_keys::CODE).await;
    let _ = session.remove::<String>(email_change_keys::TARGET).await;
    let _ = session.remove::<i64>(email_change_keys::EXPIRES).await;

    // Parse and update email
    let new_email =
        Email::parse(&target_email).map_err(|_| ApiError::new("Invalid email address"))?;

    let repo = AdminUserRepository::new(state.pool());
    let updated_user = repo
        .update_email(admin.id, &new_email)
        .await
        .map_err(|e| match e {
            crate::db::RepositoryError::Conflict(_) => {
                ApiError::new("This email address is already in use")
            }
            other => ApiError::new(format!("Failed to update email: {other}")),
        })?;

    // Update session
    let current_admin = CurrentAdmin {
        id: updated_user.id,
        email: updated_user.email.clone(),
        name: updated_user.name,
        role: updated_user.role,
    };
    set_current_admin(&session, &current_admin)
        .await
        .map_err(|e| ApiError::new(format!("Session error: {e}")))?;

    Ok(Json(VerifyEmailResponse {
        success: true,
        email: updated_user.email.to_string(),
    }))
}

// =============================================================================
// Passkey API
// =============================================================================

/// Delete a passkey.
///
/// DELETE /api/settings/passkeys/{id}
#[instrument(skip(state))]
async fn delete_passkey(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<i32>,
) -> Result<Json<DeletePasskeyResponse>, ApiError> {
    let credential_id = AdminCredentialId::new(id);

    let auth = AdminAuthService::new(state.pool(), state.webauthn());
    auth.delete_credential(admin.id, credential_id)
        .await
        .map_err(|e| match e {
            crate::services::AdminAuthError::LastCredential => {
                ApiError::new("Cannot delete your only passkey")
            }
            crate::services::AdminAuthError::CredentialNotFound => {
                ApiError::new("Passkey not found")
            }
            other => ApiError::new(format!("Failed to delete passkey: {other}")),
        })?;

    Ok(Json(DeletePasskeyResponse { success: true }))
}

//! Authentication route handlers.
//!
//! Handles login, registration, password reset, and account activation
//! via Shopify Storefront API customer authentication.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Form,
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tower_sessions::Session;
use tracing::{debug, info, instrument, warn};

use crate::config::{AnalyticsConfig, AnalyticsUserInfo};
use crate::filters;
use crate::middleware::{clear_current_customer, set_current_customer};
use crate::models::CurrentCustomer;
use crate::state::AppState;

// =============================================================================
// Password Validation
// =============================================================================

/// Minimum password length.
const MIN_PASSWORD_LENGTH: usize = 8;

/// Maximum password length to prevent denial of service via hashing.
const MAX_PASSWORD_LENGTH: usize = 128;

/// Validate password meets security requirements.
///
/// Requirements:
/// - At least 8 characters
/// - At most 128 characters
/// - At least one uppercase letter
/// - At least one lowercase letter
/// - At least one digit
fn validate_password_complexity(password: &str) -> Result<(), &'static str> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err("password_too_short");
    }

    if password.len() > MAX_PASSWORD_LENGTH {
        return Err("password_too_long");
    }

    if !password.chars().any(|c| c.is_ascii_uppercase()) {
        return Err("password_needs_uppercase");
    }

    if !password.chars().any(|c| c.is_ascii_lowercase()) {
        return Err("password_needs_lowercase");
    }

    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err("password_needs_number");
    }

    Ok(())
}

// =============================================================================
// Form Types
// =============================================================================

/// Login form data.
#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
}

/// Registration form data.
#[derive(Debug, Deserialize)]
pub struct RegisterForm {
    pub email: String,
    pub password: String,
    pub password_confirm: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

/// Forgot password form data.
#[derive(Debug, Deserialize)]
pub struct ForgotPasswordForm {
    pub email: String,
}

/// Reset password form data.
#[derive(Debug, Deserialize)]
pub struct ResetPasswordForm {
    pub password: String,
    pub password_confirm: String,
}

/// Activation form data.
#[derive(Debug, Deserialize)]
pub struct ActivateForm {
    pub password: String,
    pub password_confirm: String,
}

// =============================================================================
// Query Types
// =============================================================================

/// Query parameters for error/success display.
#[derive(Debug, Deserialize)]
pub struct MessageQuery {
    pub error: Option<String>,
    pub success: Option<String>,
}

/// Query parameters for activation/reset callbacks.
#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    /// The full Shopify URL for activation or reset
    pub url: Option<String>,
    pub error: Option<String>,
}

// =============================================================================
// Templates
// =============================================================================

/// Login page template.
#[derive(Template, WebTemplate)]
#[template(path = "auth/login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
    pub success: Option<String>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub nonce: String,
}

/// Register page template.
#[derive(Template, WebTemplate)]
#[template(path = "auth/register.html")]
pub struct RegisterTemplate {
    pub error: Option<String>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub nonce: String,
}

/// Registration success page template.
#[derive(Template, WebTemplate)]
#[template(path = "auth/register_success.html")]
pub struct RegisterSuccessTemplate {
    pub email: String,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub nonce: String,
}

/// Forgot password page template.
#[derive(Template, WebTemplate)]
#[template(path = "auth/forgot_password.html")]
pub struct ForgotPasswordTemplate {
    pub error: Option<String>,
    pub success: Option<String>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub nonce: String,
}

/// Reset password page template.
#[derive(Template, WebTemplate)]
#[template(path = "auth/reset_password.html")]
pub struct ResetPasswordTemplate {
    pub error: Option<String>,
    pub reset_url: String,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub nonce: String,
}

/// Activate account page template.
#[derive(Template, WebTemplate)]
#[template(path = "auth/activate.html")]
pub struct ActivateTemplate {
    pub error: Option<String>,
    pub activation_url: String,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub nonce: String,
}

// =============================================================================
// Login Routes
// =============================================================================

/// Display the login page.
#[instrument(skip(state, nonce))]
pub async fn login_page(
    State(state): State<AppState>,
    Query(query): Query<MessageQuery>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> impl IntoResponse {
    debug!("Rendering login page");
    LoginTemplate {
        error: query.error,
        success: query.success,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::default(),
        nonce,
    }
}

/// Handle login form submission.
///
/// Authenticates via Shopify Storefront API `customerAccessTokenCreate` mutation.
#[instrument(skip(state, session, form), fields(email = %form.email))]
pub async fn login(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<LoginForm>,
) -> Response {
    debug!("Processing login attempt");
    // Call Shopify Storefront API to create access token
    match state
        .storefront()
        .create_access_token(&form.email, &form.password)
        .await
    {
        Ok(token) => {
            debug!("Access token created, fetching customer details");
            // Fetch customer details using the token
            match state
                .storefront()
                .get_customer_by_token(&token.access_token)
                .await
            {
                Ok(customer) => {
                    let current_customer = CurrentCustomer::new(
                        customer.id,
                        customer.email.unwrap_or_default(),
                        customer.first_name,
                        customer.last_name,
                        SecretString::from(token.access_token),
                        token.expires_at,
                    );

                    // Regenerate session ID to prevent session fixation attacks
                    if let Err(e) = session.cycle_id().await {
                        tracing::error!("Failed to regenerate session ID: {}", e);
                        return Redirect::to("/auth/login?error=session").into_response();
                    }

                    if let Err(e) = set_current_customer(&session, &current_customer).await {
                        tracing::error!("Failed to set session: {}", e);
                        return Redirect::to("/auth/login?error=session").into_response();
                    }

                    info!(email = %form.email, "Customer logged in successfully");
                    Redirect::to("/account").into_response()
                }
                Err(e) => {
                    warn!("Failed to fetch customer after login: {}", e);
                    Redirect::to("/auth/login?error=customer_fetch").into_response()
                }
            }
        }
        Err(e) => {
            warn!("Login failed: {}", e);
            Redirect::to("/auth/login?error=credentials").into_response()
        }
    }
}

// =============================================================================
// Registration Routes
// =============================================================================

/// Display the registration page.
#[instrument(skip(state, nonce))]
pub async fn register_page(
    State(state): State<AppState>,
    Query(query): Query<MessageQuery>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> impl IntoResponse {
    debug!("Rendering registration page");
    RegisterTemplate {
        error: query.error,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::default(),
        nonce,
    }
}

/// Handle registration form submission.
///
/// Creates customer via Shopify Storefront API `customerCreate` mutation.
/// Shopify automatically sends an activation email.
#[instrument(skip(state, nonce, form), fields(email = %form.email))]
pub async fn register(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    Form(form): Form<RegisterForm>,
) -> Response {
    debug!("Processing registration request");
    // Validate passwords match
    if form.password != form.password_confirm {
        warn!("Registration failed: password mismatch");
        return Redirect::to("/auth/register?error=password_mismatch").into_response();
    }

    // Validate password complexity
    if let Err(msg) = validate_password_complexity(&form.password) {
        warn!("Registration failed: {}", msg);
        return Redirect::to(&format!(
            "/auth/register?error={}",
            urlencoding::encode(msg)
        ))
        .into_response();
    }

    debug!("Calling Shopify API to create customer");
    // Call Shopify Storefront API to create customer
    // Shopify will automatically send an activation email
    match state
        .storefront()
        .create_customer(
            &form.email,
            &form.password,
            form.first_name.as_deref(),
            form.last_name.as_deref(),
            false, // accepts_marketing
        )
        .await
    {
        Ok(customer) => {
            info!(email = %form.email, "Customer registered successfully, activation email sent");
            // Don't log the user in - they need to activate first
            // Show success page telling them to check their email
            RegisterSuccessTemplate {
                email: customer.email.unwrap_or_else(|| form.email.clone()),
                analytics: state.config().analytics.clone(),
                analytics_user_info: AnalyticsUserInfo::default(),
                nonce,
            }
            .into_response()
        }
        Err(e) => {
            // Log the actual error for debugging, but don't reveal to user
            warn!("Registration failed: {}", e);

            // Always show success page to prevent email enumeration
            // If email exists, Shopify won't create duplicate, but user sees same message
            // Users can use password reset if they forgot they had an account
            info!(email = %form.email, "Registration attempt (may be duplicate)");
            RegisterSuccessTemplate {
                email: form.email.clone(),
                analytics: state.config().analytics.clone(),
                analytics_user_info: AnalyticsUserInfo::default(),
                nonce,
            }
            .into_response()
        }
    }
}

// =============================================================================
// Account Activation Routes
// =============================================================================

/// Display the account activation page.
///
/// Called when user clicks the activation link in Shopify's email.
#[instrument(skip(state, nonce))]
pub async fn activate_page(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Response {
    debug!("Rendering account activation page");
    if let Some(url) = query.url {
        debug!("Activation URL provided, showing activation form");
        ActivateTemplate {
            error: query.error,
            activation_url: url,
            analytics: state.config().analytics.clone(),
            analytics_user_info: AnalyticsUserInfo::default(),
            nonce,
        }
        .into_response()
    } else {
        warn!("Activation page accessed without URL parameter");
        Redirect::to("/auth/login?error=invalid_activation_link").into_response()
    }
}

/// Handle account activation form submission.
///
/// Activates customer via Shopify Storefront API `customerActivateByUrl` mutation.
#[instrument(skip(state, session, form))]
pub async fn activate(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<CallbackQuery>,
    Form(form): Form<ActivateForm>,
) -> Response {
    debug!("Processing account activation request");
    let Some(activation_url) = query.url else {
        warn!("Activation attempted without URL parameter");
        return Redirect::to("/auth/login?error=invalid_activation_link").into_response();
    };

    // Validate passwords match
    if form.password != form.password_confirm {
        warn!("Account activation failed: password mismatch");
        let redirect_url = format!(
            "/auth/activate?url={}&error=password_mismatch",
            urlencoding::encode(&activation_url)
        );
        return Redirect::to(&redirect_url).into_response();
    }

    // Validate password complexity
    if let Err(msg) = validate_password_complexity(&form.password) {
        warn!("Account activation failed: {}", msg);
        let redirect_url = format!(
            "/auth/activate?url={}&error={}",
            urlencoding::encode(&activation_url),
            urlencoding::encode(msg)
        );
        return Redirect::to(&redirect_url).into_response();
    }

    debug!("Calling Shopify API to activate customer");
    // Call Shopify Storefront API to activate customer
    match state
        .storefront()
        .activate_customer_by_url(&activation_url, &form.password)
        .await
    {
        Ok((customer, token)) => {
            let current_customer = CurrentCustomer::new(
                customer.id,
                customer.email.unwrap_or_default(),
                customer.first_name,
                customer.last_name,
                SecretString::from(token.access_token),
                token.expires_at,
            );

            // Regenerate session ID to prevent session fixation attacks
            if let Err(e) = session.cycle_id().await {
                tracing::error!("Failed to regenerate session ID after activation: {}", e);
                return Redirect::to("/auth/login?error=session").into_response();
            }

            if let Err(e) = set_current_customer(&session, &current_customer).await {
                tracing::error!("Failed to set session after activation: {}", e);
                return Redirect::to("/auth/login?error=session").into_response();
            }

            info!("Customer account activated successfully");
            // Redirect to account page - user is now logged in
            Redirect::to("/account?activated=true").into_response()
        }
        Err(e) => {
            warn!("Account activation failed: {}", e);
            let redirect_url = format!(
                "/auth/activate?url={}&error=activation_failed",
                urlencoding::encode(&activation_url)
            );
            Redirect::to(&redirect_url).into_response()
        }
    }
}

// =============================================================================
// Password Reset Routes
// =============================================================================

/// Display the forgot password page.
#[instrument(skip(state, nonce))]
pub async fn forgot_password_page(
    State(state): State<AppState>,
    Query(query): Query<MessageQuery>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> impl IntoResponse {
    debug!("Rendering forgot password page");
    ForgotPasswordTemplate {
        error: query.error,
        success: query.success,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::default(),
        nonce,
    }
}

/// Handle forgot password form submission.
///
/// Sends recovery email via Shopify Storefront API `customerRecover` mutation.
#[instrument(skip(state, form), fields(email = %form.email))]
pub async fn forgot_password(
    State(state): State<AppState>,
    Form(form): Form<ForgotPasswordForm>,
) -> Response {
    debug!("Processing password recovery request");
    // Call Shopify Storefront API to send recovery email
    // We always show success to prevent email enumeration
    if let Err(e) = state.storefront().recover_customer(&form.email).await {
        warn!("Password recovery request failed: {}", e);
        // Still show success to prevent email enumeration
    } else {
        info!(email = %form.email, "Password recovery email sent");
    }

    Redirect::to("/auth/forgot-password?success=email_sent").into_response()
}

/// Display the reset password page.
///
/// Called when user clicks the reset link in Shopify's email.
#[instrument(skip(state, nonce))]
pub async fn reset_password_page(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Response {
    debug!("Rendering reset password page");
    if let Some(url) = query.url {
        debug!("Reset URL provided, showing reset form");
        ResetPasswordTemplate {
            error: query.error,
            reset_url: url,
            analytics: state.config().analytics.clone(),
            analytics_user_info: AnalyticsUserInfo::default(),
            nonce,
        }
        .into_response()
    } else {
        warn!("Reset password page accessed without URL parameter");
        Redirect::to("/auth/forgot-password?error=invalid_reset_link").into_response()
    }
}

/// Handle reset password form submission.
///
/// Resets password via Shopify Storefront API `customerResetByUrl` mutation.
#[instrument(skip(state, session, form))]
pub async fn reset_password(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<CallbackQuery>,
    Form(form): Form<ResetPasswordForm>,
) -> Response {
    debug!("Processing password reset request");
    let Some(reset_url) = query.url else {
        warn!("Password reset attempted without URL parameter");
        return Redirect::to("/auth/forgot-password?error=invalid_reset_link").into_response();
    };

    // Validate passwords match
    if form.password != form.password_confirm {
        warn!("Password reset failed: password mismatch");
        let redirect_url = format!(
            "/auth/reset-password?url={}&error=password_mismatch",
            urlencoding::encode(&reset_url)
        );
        return Redirect::to(&redirect_url).into_response();
    }

    // Validate password complexity
    if let Err(msg) = validate_password_complexity(&form.password) {
        warn!("Password reset failed: {}", msg);
        let redirect_url = format!(
            "/auth/reset-password?url={}&error={}",
            urlencoding::encode(&reset_url),
            urlencoding::encode(msg)
        );
        return Redirect::to(&redirect_url).into_response();
    }

    debug!("Calling Shopify API to reset password");
    // Call Shopify Storefront API to reset password
    match state
        .storefront()
        .reset_customer_by_url(&reset_url, &form.password)
        .await
    {
        Ok((customer, token)) => {
            let current_customer = CurrentCustomer::new(
                customer.id,
                customer.email.unwrap_or_default(),
                customer.first_name,
                customer.last_name,
                SecretString::from(token.access_token),
                token.expires_at,
            );

            // Regenerate session ID to prevent session fixation attacks
            if let Err(e) = session.cycle_id().await {
                tracing::error!(
                    "Failed to regenerate session ID after password reset: {}",
                    e
                );
                return Redirect::to("/auth/login?error=session").into_response();
            }

            if let Err(e) = set_current_customer(&session, &current_customer).await {
                tracing::error!("Failed to set session after password reset: {}", e);
                return Redirect::to("/auth/login?error=session").into_response();
            }

            info!("Customer password reset successfully");
            // Redirect to account page - user is now logged in
            Redirect::to("/account").into_response()
        }
        Err(e) => {
            warn!("Password reset failed: {}", e);
            let redirect_url = format!(
                "/auth/reset-password?url={}&error=reset_failed",
                urlencoding::encode(&reset_url)
            );
            Redirect::to(&redirect_url).into_response()
        }
    }
}

// =============================================================================
// Logout Route
// =============================================================================

/// Handle logout.
///
/// Clears the session and optionally deletes the Shopify access token.
#[instrument(skip(state, session))]
pub async fn logout(State(state): State<AppState>, session: Session) -> Response {
    debug!("Processing logout request");
    // Get the current customer to delete their access token
    if let Ok(Some(customer)) = session
        .get::<CurrentCustomer>(crate::models::session_keys::CURRENT_CUSTOMER)
        .await
    {
        debug!("Found customer session, deleting Shopify access token");
        // Delete the access token from Shopify (best effort)
        if let Err(e) = state
            .storefront()
            .delete_access_token(customer.access_token().expose_secret())
            .await
        {
            warn!("Failed to delete Shopify access token: {}", e);
        }
    }

    if let Err(e) = clear_current_customer(&session).await {
        tracing::error!("Failed to clear session: {}", e);
    }

    // Also destroy the entire session
    if let Err(e) = session.flush().await {
        tracing::error!("Failed to flush session: {}", e);
    }

    info!("Customer logged out successfully");
    Redirect::to("/?logged_out=1").into_response()
}

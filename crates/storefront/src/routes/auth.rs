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

use crate::filters;
use crate::middleware::{clear_current_customer, set_current_customer};
use crate::models::CurrentCustomer;
use crate::state::AppState;

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
}

/// Register page template.
#[derive(Template, WebTemplate)]
#[template(path = "auth/register.html")]
pub struct RegisterTemplate {
    pub error: Option<String>,
}

/// Registration success page template.
#[derive(Template, WebTemplate)]
#[template(path = "auth/register_success.html")]
pub struct RegisterSuccessTemplate {
    pub email: String,
}

/// Forgot password page template.
#[derive(Template, WebTemplate)]
#[template(path = "auth/forgot_password.html")]
pub struct ForgotPasswordTemplate {
    pub error: Option<String>,
    pub success: Option<String>,
}

/// Reset password page template.
#[derive(Template, WebTemplate)]
#[template(path = "auth/reset_password.html")]
pub struct ResetPasswordTemplate {
    pub error: Option<String>,
    pub reset_url: String,
}

/// Activate account page template.
#[derive(Template, WebTemplate)]
#[template(path = "auth/activate.html")]
pub struct ActivateTemplate {
    pub error: Option<String>,
    pub activation_url: String,
}

// =============================================================================
// Login Routes
// =============================================================================

/// Display the login page.
pub async fn login_page(Query(query): Query<MessageQuery>) -> impl IntoResponse {
    LoginTemplate {
        error: query.error,
        success: query.success,
    }
}

/// Handle login form submission.
///
/// Authenticates via Shopify Storefront API `customerAccessTokenCreate` mutation.
pub async fn login(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<LoginForm>,
) -> Response {
    // Call Shopify Storefront API to create access token
    match state
        .storefront()
        .create_access_token(&form.email, &form.password)
        .await
    {
        Ok(token) => {
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

                    if let Err(e) = set_current_customer(&session, &current_customer).await {
                        tracing::error!("Failed to set session: {}", e);
                        return Redirect::to("/auth/login?error=session").into_response();
                    }

                    Redirect::to("/account").into_response()
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch customer after login: {}", e);
                    Redirect::to("/auth/login?error=customer_fetch").into_response()
                }
            }
        }
        Err(e) => {
            tracing::warn!("Login failed: {}", e);
            Redirect::to("/auth/login?error=credentials").into_response()
        }
    }
}

// =============================================================================
// Registration Routes
// =============================================================================

/// Display the registration page.
pub async fn register_page(Query(query): Query<MessageQuery>) -> impl IntoResponse {
    RegisterTemplate { error: query.error }
}

/// Handle registration form submission.
///
/// Creates customer via Shopify Storefront API `customerCreate` mutation.
/// Shopify automatically sends an activation email.
pub async fn register(
    State(state): State<AppState>,
    Form(form): Form<RegisterForm>,
) -> Response {
    // Validate passwords match
    if form.password != form.password_confirm {
        return Redirect::to("/auth/register?error=password_mismatch").into_response();
    }

    // Validate password length
    if form.password.len() < 8 {
        return Redirect::to("/auth/register?error=password_too_short").into_response();
    }

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
            // Don't log the user in - they need to activate first
            // Show success page telling them to check their email
            RegisterSuccessTemplate {
                email: customer.email.unwrap_or_else(|| form.email.clone()),
            }
            .into_response()
        }
        Err(e) => {
            tracing::warn!("Registration failed: {}", e);
            // Check for specific error types
            let error_msg = e.to_string();
            if error_msg.contains("taken") || error_msg.contains("already") {
                Redirect::to("/auth/register?error=email_taken").into_response()
            } else {
                Redirect::to("/auth/register?error=failed").into_response()
            }
        }
    }
}

// =============================================================================
// Account Activation Routes
// =============================================================================

/// Display the account activation page.
///
/// Called when user clicks the activation link in Shopify's email.
pub async fn activate_page(Query(query): Query<CallbackQuery>) -> Response {
    match query.url {
        Some(url) => ActivateTemplate {
            error: query.error,
            activation_url: url,
        }
        .into_response(),
        None => Redirect::to("/auth/login?error=invalid_activation_link").into_response(),
    }
}

/// Handle account activation form submission.
///
/// Activates customer via Shopify Storefront API `customerActivateByUrl` mutation.
pub async fn activate(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<CallbackQuery>,
    Form(form): Form<ActivateForm>,
) -> Response {
    let activation_url = match query.url {
        Some(url) => url,
        None => return Redirect::to("/auth/login?error=invalid_activation_link").into_response(),
    };

    // Validate passwords match
    if form.password != form.password_confirm {
        let redirect_url = format!(
            "/auth/activate?url={}&error=password_mismatch",
            urlencoding::encode(&activation_url)
        );
        return Redirect::to(&redirect_url).into_response();
    }

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

            if let Err(e) = set_current_customer(&session, &current_customer).await {
                tracing::error!("Failed to set session after activation: {}", e);
                return Redirect::to("/auth/login?error=session").into_response();
            }

            // Redirect to account page - user is now logged in
            Redirect::to("/account?activated=true").into_response()
        }
        Err(e) => {
            tracing::warn!("Account activation failed: {}", e);
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
pub async fn forgot_password_page(Query(query): Query<MessageQuery>) -> impl IntoResponse {
    ForgotPasswordTemplate {
        error: query.error,
        success: query.success,
    }
}

/// Handle forgot password form submission.
///
/// Sends recovery email via Shopify Storefront API `customerRecover` mutation.
pub async fn forgot_password(
    State(state): State<AppState>,
    Form(form): Form<ForgotPasswordForm>,
) -> Response {
    // Call Shopify Storefront API to send recovery email
    // We always show success to prevent email enumeration
    if let Err(e) = state.storefront().recover_customer(&form.email).await {
        tracing::warn!("Password recovery request failed: {}", e);
        // Still show success to prevent email enumeration
    }

    Redirect::to("/auth/forgot-password?success=email_sent").into_response()
}

/// Display the reset password page.
///
/// Called when user clicks the reset link in Shopify's email.
pub async fn reset_password_page(Query(query): Query<CallbackQuery>) -> Response {
    match query.url {
        Some(url) => ResetPasswordTemplate {
            error: query.error,
            reset_url: url,
        }
        .into_response(),
        None => Redirect::to("/auth/forgot-password?error=invalid_reset_link").into_response(),
    }
}

/// Handle reset password form submission.
///
/// Resets password via Shopify Storefront API `customerResetByUrl` mutation.
pub async fn reset_password(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<CallbackQuery>,
    Form(form): Form<ResetPasswordForm>,
) -> Response {
    let reset_url = match query.url {
        Some(url) => url,
        None => return Redirect::to("/auth/forgot-password?error=invalid_reset_link").into_response(),
    };

    // Validate passwords match
    if form.password != form.password_confirm {
        let redirect_url = format!(
            "/auth/reset-password?url={}&error=password_mismatch",
            urlencoding::encode(&reset_url)
        );
        return Redirect::to(&redirect_url).into_response();
    }

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

            if let Err(e) = set_current_customer(&session, &current_customer).await {
                tracing::error!("Failed to set session after password reset: {}", e);
                return Redirect::to("/auth/login?error=session").into_response();
            }

            // Redirect to account page - user is now logged in
            Redirect::to("/account").into_response()
        }
        Err(e) => {
            tracing::warn!("Password reset failed: {}", e);
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
pub async fn logout(State(state): State<AppState>, session: Session) -> Response {
    // Get the current customer to delete their access token
    if let Ok(Some(customer)) = session
        .get::<CurrentCustomer>(crate::models::session_keys::CURRENT_CUSTOMER)
        .await
    {
        // Delete the access token from Shopify (best effort)
        if let Err(e) = state
            .storefront()
            .delete_access_token(customer.access_token().expose_secret())
            .await
        {
            tracing::warn!("Failed to delete Shopify access token: {}", e);
        }
    }

    if let Err(e) = clear_current_customer(&session).await {
        tracing::error!("Failed to clear session: {}", e);
    }

    // Also destroy the entire session
    if let Err(e) = session.flush().await {
        tracing::error!("Failed to flush session: {}", e);
    }

    Redirect::to("/").into_response()
}

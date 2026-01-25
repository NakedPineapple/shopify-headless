//! Authentication route handlers.
//!
//! Handles login, registration, and logout for HTML form submissions.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Form,
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::filters;
use crate::middleware::{clear_current_user, set_current_user};
use crate::models::CurrentUser;
use crate::services::AuthService;
use crate::state::AppState;

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
}

/// Query parameters for error display.
#[derive(Debug, Deserialize)]
pub struct ErrorQuery {
    pub error: Option<String>,
}

/// Login page template.
#[derive(Template, WebTemplate)]
#[template(path = "auth/login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
}

/// Register page template.
#[derive(Template, WebTemplate)]
#[template(path = "auth/register.html")]
pub struct RegisterTemplate {
    pub error: Option<String>,
}

/// Display the login page.
pub async fn login_page(Query(query): Query<ErrorQuery>) -> impl IntoResponse {
    LoginTemplate { error: query.error }
}

/// Handle login form submission.
pub async fn login(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<LoginForm>,
) -> Response {
    let auth = AuthService::new(state.pool(), state.webauthn());

    match auth.login_with_password(&form.email, &form.password).await {
        Ok(user) => {
            let current_user = CurrentUser {
                id: user.id,
                email: user.email,
            };

            if let Err(e) = set_current_user(&session, &current_user).await {
                tracing::error!("Failed to set session: {}", e);
                return Redirect::to("/auth/login?error=session").into_response();
            }

            Redirect::to("/account").into_response()
        }
        Err(e) => {
            tracing::warn!("Login failed: {}", e);
            Redirect::to("/auth/login?error=credentials").into_response()
        }
    }
}

/// Display the registration page.
pub async fn register_page(Query(query): Query<ErrorQuery>) -> impl IntoResponse {
    RegisterTemplate { error: query.error }
}

/// Handle registration form submission.
pub async fn register(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<RegisterForm>,
) -> Response {
    // Validate passwords match
    if form.password != form.password_confirm {
        return Redirect::to("/auth/register?error=password_mismatch").into_response();
    }

    let auth = AuthService::new(state.pool(), state.webauthn());

    match auth
        .register_with_password(&form.email, &form.password)
        .await
    {
        Ok(user) => {
            let current_user = CurrentUser {
                id: user.id,
                email: user.email,
            };

            if let Err(e) = set_current_user(&session, &current_user).await {
                tracing::error!("Failed to set session: {}", e);
                return Redirect::to("/auth/login").into_response();
            }

            // TODO: Send verification email
            Redirect::to("/account").into_response()
        }
        Err(e) => {
            tracing::warn!("Registration failed: {}", e);
            Redirect::to("/auth/register?error=failed").into_response()
        }
    }
}

/// Handle logout.
pub async fn logout(session: Session) -> Response {
    if let Err(e) = clear_current_user(&session).await {
        tracing::error!("Failed to clear session: {}", e);
    }

    // Also destroy the entire session
    if let Err(e) = session.flush().await {
        tracing::error!("Failed to flush session: {}", e);
    }

    Redirect::to("/").into_response()
}

//! Authentication route handlers for admin.
//!
//! Provides login page (passkey + break-glass password) and logout functionality.

use askama::Template;
use axum::{
    Form, Router,
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use secrecy::SecretString;
use serde::Deserialize;
use tower_sessions::Session;
use tracing::{debug, info, instrument, warn};

use crate::error::{clear_sentry_user, set_sentry_user};
use crate::filters;
use crate::middleware::{clear_current_admin, set_current_admin};
use crate::models::CurrentAdmin;
use crate::services::AdminAuthService;
use crate::state::AppState;

/// Login page template.
#[derive(Template)]
#[template(path = "auth/login.html")]
struct LoginPageTemplate<'a> {
    primary_origin: &'a str,
    show_password: bool,
    error_message: Option<String>,
}

/// Query parameters for login page.
#[derive(Debug, Deserialize)]
struct LoginQuery {
    /// If present, shows the break-glass password form.
    #[serde(default)]
    emergency: Option<String>,
}

/// Password login form data.
#[derive(Debug, Deserialize)]
struct PasswordLoginForm {
    email: String,
    password: String,
}

/// Build the auth router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", get(login_page))
        .route("/auth/login/password", post(password_login))
        .route("/auth/logout", post(logout))
}

/// Render the login page.
///
/// GET /auth/login
///
/// Accepts `?emergency` query parameter to show the break-glass password form.
#[instrument(skip(state))]
async fn login_page(
    State(state): State<AppState>,
    Query(query): Query<LoginQuery>,
) -> impl IntoResponse {
    debug!(
        show_password = query.emergency.is_some(),
        "Rendering login page"
    );
    let primary_origin = state.config().primary_origin();
    Html(
        LoginPageTemplate {
            primary_origin: &primary_origin,
            show_password: query.emergency.is_some(),
            error_message: None,
        }
        .render()
        .unwrap_or_else(|_| String::from("Error rendering template")),
    )
}

/// Break-glass password authentication.
///
/// POST /auth/login/password
#[instrument(skip(state, session, form))]
async fn password_login(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<PasswordLoginForm>,
) -> impl IntoResponse {
    // Wrap password in SecretString immediately so it is zeroized on drop
    let password = SecretString::from(form.password);

    let auth = AdminAuthService::new(state.pool(), state.webauthn());

    match auth
        .authenticate_with_password(&form.email, &password)
        .await
    {
        Ok(user) => {
            let current_admin = CurrentAdmin {
                id: user.id,
                email: user.email,
                name: user.name,
                role: user.role,
                slack_user_id: user.slack_user_id,
            };

            if let Err(e) = set_current_admin(&session, &current_admin).await {
                warn!(error = %e, "Failed to create admin session after password auth");
                let primary_origin = state.config().primary_origin();
                return Html(
                    LoginPageTemplate {
                        primary_origin: &primary_origin,
                        show_password: true,
                        error_message: Some("Session error. Please try again.".to_owned()),
                    }
                    .render()
                    .unwrap_or_else(|_| String::from("Error rendering template")),
                )
                .into_response();
            }

            set_sentry_user(
                current_admin.id.as_i32(),
                Some(current_admin.email.as_str()),
            );

            info!(
                admin_id = current_admin.id.as_i32(),
                "Admin authenticated via break-glass password"
            );

            Redirect::to("/chat").into_response()
        }
        Err(e) => {
            warn!(error = %e, "Break-glass password login failed");
            let primary_origin = state.config().primary_origin();
            Html(
                LoginPageTemplate {
                    primary_origin: &primary_origin,
                    show_password: true,
                    error_message: Some("Invalid email or password".to_owned()),
                }
                .render()
                .unwrap_or_else(|_| String::from("Error rendering template")),
            )
            .into_response()
        }
    }
}

/// Logout and clear session.
///
/// POST /auth/logout
#[instrument(skip(session))]
async fn logout(session: Session) -> impl IntoResponse {
    debug!("Processing logout request");

    // Clear the current admin from session
    match clear_current_admin(&session).await {
        Ok(()) => info!("Admin session cleared"),
        Err(e) => warn!(error = %e, "Failed to clear admin session"),
    }

    clear_sentry_user();

    // Redirect to login page
    Redirect::to("/auth/login")
}

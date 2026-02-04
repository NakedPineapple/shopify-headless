//! Authentication route handlers for admin.
//!
//! Provides login page and logout functionality.
//! No password form - passkey only.

use askama::Template;
use axum::{
    Router,
    extract::State,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use tower_sessions::Session;
use tracing::{debug, info, instrument, warn};

use crate::error::clear_sentry_user;
use crate::filters;
use crate::middleware::clear_current_admin;
use crate::state::AppState;

/// Login page template.
#[derive(Template)]
#[template(path = "auth/login.html")]
struct LoginPageTemplate;

/// Build the auth router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", get(login_page))
        .route("/auth/logout", post(logout))
}

/// Render the login page.
///
/// GET /auth/login
#[instrument]
async fn login_page() -> impl IntoResponse {
    debug!("Rendering login page");
    Html(
        LoginPageTemplate
            .render()
            .unwrap_or_else(|_| String::from("Error rendering template")),
    )
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

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
async fn login_page() -> impl IntoResponse {
    Html(
        LoginPageTemplate
            .render()
            .unwrap_or_else(|_| String::from("Error rendering template")),
    )
}

/// Logout and clear session.
///
/// POST /auth/logout
async fn logout(session: Session) -> impl IntoResponse {
    // Clear the current admin from session
    let _ = clear_current_admin(&session).await;

    // Redirect to login page
    Redirect::to("/auth/login")
}

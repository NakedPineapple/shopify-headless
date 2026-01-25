//! Static content page route handlers.
//!
//! Serves markdown-based content pages like terms, privacy, FAQ, etc.

use askama::Template;
use askama_web::WebTemplate;
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use chrono::NaiveDate;
use tracing::instrument;

use crate::filters;
use crate::state::AppState;

/// Content page template.
#[derive(Template, WebTemplate)]
#[template(path = "pages/content.html")]
pub struct ContentPageTemplate {
    pub title: String,
    pub description: String,
    pub updated_at: Option<NaiveDate>,
    pub content_html: String,
}

/// Serve a content page by slug.
fn serve_content_page(state: &AppState, slug: &str) -> Result<ContentPageTemplate, StatusCode> {
    let page = state
        .content()
        .get_page(slug)
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(ContentPageTemplate {
        title: page.meta.title.clone(),
        description: page.meta.description.clone().unwrap_or_default(),
        updated_at: page.meta.updated_at,
        content_html: page.content_html.clone(),
    })
}

/// Display the Terms of Service page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state))]
pub async fn terms(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    serve_content_page(&state, "terms")
}

/// Display the Privacy Policy page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state))]
pub async fn privacy(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    serve_content_page(&state, "privacy")
}

/// Display the Accessibility page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state))]
pub async fn accessibility(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    serve_content_page(&state, "accessibility")
}

/// Display the FAQ page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state))]
pub async fn faq(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    serve_content_page(&state, "faq")
}

/// Display the Shipping & Returns page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state))]
pub async fn shipping(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    serve_content_page(&state, "shipping")
}

/// Create the pages routes router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/terms", get(terms))
        .route("/privacy", get(privacy))
        .route("/accessibility", get(accessibility))
        .route("/faq", get(faq))
        .route("/shipping", get(shipping))
}

//! Static content page route handlers.
//!
//! Serves both template-based pages (about, programs) and markdown-based
//! content pages (terms, privacy, FAQ, etc.)

use askama::Template;
use askama_web::WebTemplate;
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use chrono::NaiveDate;
use tracing::instrument;

use crate::config::AnalyticsConfig;
use crate::filters;
use crate::state::AppState;

// =============================================================================
// Template-based Pages (About, Programs, etc.)
// =============================================================================

/// About page template.
#[derive(Template, WebTemplate)]
#[template(path = "pages/about.html")]
pub struct AboutTemplate {
    pub analytics: AnalyticsConfig,
}

/// Wholesale page template.
#[derive(Template, WebTemplate)]
#[template(path = "pages/wholesale.html")]
pub struct WholesaleTemplate {
    pub analytics: AnalyticsConfig,
}

/// Model Program page template.
#[derive(Template, WebTemplate)]
#[template(path = "pages/model_program.html")]
pub struct ModelProgramTemplate {
    pub analytics: AnalyticsConfig,
}

/// Affiliate Program page template.
#[derive(Template, WebTemplate)]
#[template(path = "pages/affiliate.html")]
pub struct AffiliateTemplate {
    pub analytics: AnalyticsConfig,
}

/// Teen Program page template.
#[derive(Template, WebTemplate)]
#[template(path = "pages/teen_program.html")]
pub struct TeenProgramTemplate {
    pub analytics: AnalyticsConfig,
}

/// Subscriptions page template.
#[derive(Template, WebTemplate)]
#[template(path = "pages/subscriptions.html")]
pub struct SubscriptionsTemplate {
    pub analytics: AnalyticsConfig,
}

// =============================================================================
// Markdown-based Content Pages
// =============================================================================

/// Content page template for markdown-based pages.
#[derive(Template, WebTemplate)]
#[template(path = "pages/content.html")]
pub struct ContentPageTemplate {
    pub title: String,
    pub description: String,
    pub updated_at: Option<NaiveDate>,
    pub content_html: String,
    pub analytics: AnalyticsConfig,
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
        analytics: state.config().analytics.clone(),
    })
}

// =============================================================================
// Template-based Route Handlers
// =============================================================================

/// Display the About page.
pub async fn about(State(state): State<AppState>) -> AboutTemplate {
    AboutTemplate {
        analytics: state.config().analytics.clone(),
    }
}

/// Display the Wholesale page.
pub async fn wholesale(State(state): State<AppState>) -> WholesaleTemplate {
    WholesaleTemplate {
        analytics: state.config().analytics.clone(),
    }
}

/// Display the Model Program page.
pub async fn model_program(State(state): State<AppState>) -> ModelProgramTemplate {
    ModelProgramTemplate {
        analytics: state.config().analytics.clone(),
    }
}

/// Display the Affiliate Program page.
pub async fn affiliate_program(State(state): State<AppState>) -> AffiliateTemplate {
    AffiliateTemplate {
        analytics: state.config().analytics.clone(),
    }
}

/// Display the Teen Program page.
pub async fn teen_program(State(state): State<AppState>) -> TeenProgramTemplate {
    TeenProgramTemplate {
        analytics: state.config().analytics.clone(),
    }
}

/// Display the Subscriptions page.
pub async fn subscriptions(State(state): State<AppState>) -> SubscriptionsTemplate {
    SubscriptionsTemplate {
        analytics: state.config().analytics.clone(),
    }
}

// =============================================================================
// Markdown-based Route Handlers
// =============================================================================

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

/// Display the Data Sharing Opt-Out page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state))]
pub async fn data_sharing_opt_out(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    serve_content_page(&state, "data-sharing-opt-out")
}

/// Display the Directions page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state))]
pub async fn directions(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    serve_content_page(&state, "directions")
}

/// Display the Collabs page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state))]
pub async fn collabs(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    serve_content_page(&state, "collabs")
}

// =============================================================================
// Router
// =============================================================================

/// Create the pages routes router.
pub fn router() -> Router<AppState> {
    Router::new()
        // Template-based pages
        .route("/pages/about", get(about))
        .route("/pages/wholesale", get(wholesale))
        .route("/pages/model-program", get(model_program))
        .route("/pages/affiliate-program", get(affiliate_program))
        .route("/pages/teen-program", get(teen_program))
        .route("/pages/subscriptions", get(subscriptions))
        // Markdown-based content pages
        .route("/terms", get(terms))
        .route("/privacy", get(privacy))
        .route("/accessibility", get(accessibility))
        .route("/faq", get(faq))
        .route("/shipping", get(shipping))
        .route("/privacy/data-sharing-opt-out", get(data_sharing_opt_out))
        .route("/directions", get(directions))
        .route("/collabs", get(collabs))
}

//! Static content page route handlers.
//!
//! Serves both template-based pages (about, programs) and markdown-based
//! content pages (terms, privacy, FAQ, etc.)

use askama::Template;
use askama_web::WebTemplate;
use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use chrono::NaiveDate;
use tracing::{debug, info, instrument, warn};

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
    pub nonce: String,
}

/// Wholesale page template.
#[derive(Template, WebTemplate)]
#[template(path = "pages/wholesale.html")]
pub struct WholesaleTemplate {
    pub analytics: AnalyticsConfig,
    pub nonce: String,
}

/// Model Program page template.
#[derive(Template, WebTemplate)]
#[template(path = "pages/model_program.html")]
pub struct ModelProgramTemplate {
    pub analytics: AnalyticsConfig,
    pub nonce: String,
}

/// Affiliate Program page template.
#[derive(Template, WebTemplate)]
#[template(path = "pages/affiliate.html")]
pub struct AffiliateTemplate {
    pub analytics: AnalyticsConfig,
    pub nonce: String,
}

/// Teen Program page template.
#[derive(Template, WebTemplate)]
#[template(path = "pages/teen_program.html")]
pub struct TeenProgramTemplate {
    pub analytics: AnalyticsConfig,
    pub nonce: String,
}

/// Subscriptions page template.
#[derive(Template, WebTemplate)]
#[template(path = "pages/subscriptions.html")]
pub struct SubscriptionsTemplate {
    pub analytics: AnalyticsConfig,
    pub nonce: String,
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
    pub nonce: String,
}

/// Serve a content page by slug.
fn serve_content_page(
    state: &AppState,
    slug: &str,
    nonce: String,
) -> Result<ContentPageTemplate, StatusCode> {
    debug!(slug = %slug, "Looking up content page by slug");

    let page = state.content().get_page(slug).ok_or_else(|| {
        warn!(slug = %slug, "Content page not found");
        StatusCode::NOT_FOUND
    })?;

    info!(slug = %slug, title = %page.meta.title, "Serving content page");

    Ok(ContentPageTemplate {
        title: page.meta.title.clone(),
        description: page.meta.description.clone().unwrap_or_default(),
        updated_at: page.meta.updated_at,
        content_html: page.content_html.clone(),
        analytics: state.config().analytics.clone(),
        nonce,
    })
}

// =============================================================================
// Template-based Route Handlers
// =============================================================================

/// Display the About page.
#[instrument(skip(state, nonce))]
pub async fn about(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> AboutTemplate {
    debug!("Rendering About page");
    info!("Serving About page");
    AboutTemplate {
        analytics: state.config().analytics.clone(),
        nonce,
    }
}

/// Display the Wholesale page.
#[instrument(skip(state, nonce))]
pub async fn wholesale(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> WholesaleTemplate {
    debug!("Rendering Wholesale page");
    info!("Serving Wholesale page");
    WholesaleTemplate {
        analytics: state.config().analytics.clone(),
        nonce,
    }
}

/// Display the Model Program page.
#[instrument(skip(state, nonce))]
pub async fn model_program(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> ModelProgramTemplate {
    debug!("Rendering Model Program page");
    info!("Serving Model Program page");
    ModelProgramTemplate {
        analytics: state.config().analytics.clone(),
        nonce,
    }
}

/// Display the Affiliate Program page.
#[instrument(skip(state, nonce))]
pub async fn affiliate_program(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> AffiliateTemplate {
    debug!("Rendering Affiliate Program page");
    info!("Serving Affiliate Program page");
    AffiliateTemplate {
        analytics: state.config().analytics.clone(),
        nonce,
    }
}

/// Display the Teen Program page.
#[instrument(skip(state, nonce))]
pub async fn teen_program(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> TeenProgramTemplate {
    debug!("Rendering Teen Program page");
    info!("Serving Teen Program page");
    TeenProgramTemplate {
        analytics: state.config().analytics.clone(),
        nonce,
    }
}

/// Display the Subscriptions page.
#[instrument(skip(state, nonce))]
pub async fn subscriptions(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> SubscriptionsTemplate {
    debug!("Rendering Subscriptions page");
    info!("Serving Subscriptions page");
    SubscriptionsTemplate {
        analytics: state.config().analytics.clone(),
        nonce,
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
#[instrument(skip(state, nonce))]
pub async fn terms(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Result<impl IntoResponse, StatusCode> {
    debug!("Handling request for Terms of Service page");
    serve_content_page(&state, "terms", nonce)
}

/// Display the Privacy Policy page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state, nonce))]
pub async fn privacy(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Result<impl IntoResponse, StatusCode> {
    debug!("Handling request for Privacy Policy page");
    serve_content_page(&state, "privacy", nonce)
}

/// Display the Accessibility page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state, nonce))]
pub async fn accessibility(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Result<impl IntoResponse, StatusCode> {
    debug!("Handling request for Accessibility page");
    serve_content_page(&state, "accessibility", nonce)
}

/// Display the FAQ page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state, nonce))]
pub async fn faq(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Result<impl IntoResponse, StatusCode> {
    debug!("Handling request for FAQ page");
    serve_content_page(&state, "faq", nonce)
}

/// Display the Shipping & Returns page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state, nonce))]
pub async fn shipping(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Result<impl IntoResponse, StatusCode> {
    debug!("Handling request for Shipping & Returns page");
    serve_content_page(&state, "shipping", nonce)
}

/// Display the Data Sharing Opt-Out page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state, nonce))]
pub async fn data_sharing_opt_out(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Result<impl IntoResponse, StatusCode> {
    debug!("Handling request for Data Sharing Opt-Out page");
    serve_content_page(&state, "data-sharing-opt-out", nonce)
}

/// Display the Directions page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state, nonce))]
pub async fn directions(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Result<impl IntoResponse, StatusCode> {
    debug!("Handling request for Directions page");
    serve_content_page(&state, "directions", nonce)
}

/// Display the Collabs page.
///
/// # Errors
///
/// Returns 404 if the page doesn't exist.
#[instrument(skip(state, nonce))]
pub async fn collabs(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Result<impl IntoResponse, StatusCode> {
    debug!("Handling request for Collabs page");
    serve_content_page(&state, "collabs", nonce)
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

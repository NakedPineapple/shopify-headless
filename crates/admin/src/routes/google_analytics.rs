//! Google Merchant Center analytics route handlers.
//!
//! Provides a catalog health dashboard showing connection status,
//! product coverage, and mapping statistics. Like Pinterest, Google
//! is discovery-only — purchases happen on the Shopify storefront.

use askama::Template;
use axum::{extract::State, response::Html};
use tracing::{debug, instrument};

use crate::db::GoogleProductMappingRepository;
use crate::filters;
use crate::middleware::auth::RequireAdminAuth;
use crate::state::AppState;

use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Google catalog health dashboard template.
#[derive(Template)]
#[template(path = "analytics/google.html")]
struct GoogleAnalyticsTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
    mapping_count: i64,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// Google catalog health dashboard.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn google_analytics(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
) -> Html<String> {
    debug!("Fetching Google analytics dashboard");

    let connected = state.google().is_some();
    let repo = GoogleProductMappingRepository::new(state.pool());
    let mapping_count = repo.count().await.unwrap_or(0);

    let template = GoogleAnalyticsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/analytics/google".to_string(),
        connected,
        mapping_count,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {e}");
        "Internal Server Error".to_string()
    }))
}

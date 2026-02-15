//! Pinterest analytics route handlers.
//!
//! Provides a catalog health dashboard showing feed processing status,
//! product coverage, and mapping statistics. Unlike Amazon/Meta/TikTok,
//! Pinterest has no orders — purchases happen on the Shopify storefront.

use askama::Template;
use axum::{extract::State, response::Html};
use tracing::{debug, instrument};

use crate::db::PinterestProductMappingRepository;
use crate::filters;
use crate::middleware::auth::RequireAdminAuth;
use crate::state::AppState;

use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Pinterest catalog health dashboard template.
#[derive(Template)]
#[template(path = "analytics/pinterest.html")]
struct PinterestAnalyticsTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
    mapping_count: i64,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// Pinterest catalog health dashboard.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn pinterest_analytics(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
) -> Html<String> {
    debug!("Fetching Pinterest analytics dashboard");

    let connected = state.pinterest().is_some();
    let repo = PinterestProductMappingRepository::new(state.pool());
    let mapping_count = repo.count().await.unwrap_or(0);

    let template = PinterestAnalyticsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/analytics/pinterest".to_string(),
        connected,
        mapping_count,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {e}");
        "Internal Server Error".to_string()
    }))
}

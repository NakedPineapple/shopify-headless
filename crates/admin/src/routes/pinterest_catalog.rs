//! Pinterest catalog search and product mapping routes.
//!
//! These routes provide catalog browsing via the Pinterest API v5
//! and CRUD operations for Shopify-to-Pinterest product mappings.
//! Only `super_admin` users can access these features.

use askama::Template;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use tracing::instrument;

use naked_pineapple_services::pinterest::{PinterestCatalogItem, PinterestClient, PinterestError};

use crate::db::{
    CreatePinterestMappingParams, PinterestProductMapping, PinterestProductMappingRepository,
};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::amazon_settings::ApiResponse;
use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Pinterest catalog search page.
#[derive(Template)]
#[template(path = "pinterest/catalog.html")]
struct PinterestCatalogTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
}

/// Pinterest product mappings page.
#[derive(Template)]
#[template(path = "pinterest/mappings.html")]
struct PinterestMappingsTemplate {
    admin_user: AdminUserView,
    current_path: String,
    mappings: Vec<PinterestProductMapping>,
    mapping_count: usize,
}

/// HTMX partial: catalog search results.
#[derive(Template)]
#[template(path = "pinterest/partials/search_results.html")]
struct PinterestSearchResultsTemplate {
    items: Vec<PinterestCatalogItem>,
    total: usize,
    has_next: bool,
    next_bookmark: Option<String>,
    error: Option<String>,
}

/// HTMX partial: single mapping row.
#[derive(Template)]
#[template(path = "pinterest/partials/mapping_row.html")]
struct PinterestMappingRowTemplate {
    mapping: PinterestProductMapping,
}

// =============================================================================
// Router
// =============================================================================

/// Build the Pinterest catalog router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/pinterest/catalog", get(catalog_page))
        .route("/pinterest/catalog/search", get(catalog_search))
        .route("/pinterest/mappings", get(mappings_page))
        .route("/pinterest/mappings/create", post(create_mapping))
        .route("/pinterest/mappings/{id}/delete", post(delete_mapping))
        .route("/pinterest/mappings/auto-match", post(auto_match))
}

// =============================================================================
// Query Parameters
// =============================================================================

#[derive(Debug, Deserialize)]
struct CatalogSearchParams {
    q: Option<String>,
    product_group_id: Option<String>,
    bookmark: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateMappingRequest {
    shopify_product_id: String,
    shopify_variant_id: Option<String>,
    pinterest_item_id: String,
    match_type: Option<String>,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /pinterest/catalog -- Catalog search page.
#[instrument(skip(state, session))]
async fn catalog_page(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let connected = state.pinterest().is_some();

    let template = PinterestCatalogTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/pinterest/catalog".to_string(),
        connected,
    };

    render(template)
}

/// HTMX search results partial for catalog items.
#[instrument(skip(state, session))]
async fn catalog_search(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<CatalogSearchParams>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(product_group_id) = params
        .product_group_id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
    else {
        return render(PinterestSearchResultsTemplate {
            items: vec![],
            total: 0,
            has_next: false,
            next_bookmark: None,
            error: Some("Product Group ID is required".to_string()),
        });
    };

    let Some(client) = state.pinterest() else {
        return render(PinterestSearchResultsTemplate {
            items: vec![],
            total: 0,
            has_next: false,
            next_bookmark: None,
            error: Some("Pinterest API is not connected".to_string()),
        });
    };

    match client
        .list_product_group_items(product_group_id, params.bookmark)
        .await
    {
        Ok(page) => {
            let items = page.items.unwrap_or_default();
            let has_next = page.bookmark.is_some();

            // Filter by search query if provided
            let items = if let Some(q) = params.q.as_deref().filter(|q| !q.trim().is_empty()) {
                let q_lower = q.to_lowercase();
                items
                    .into_iter()
                    .filter(|item| {
                        item.title
                            .as_deref()
                            .is_some_and(|t| t.to_lowercase().contains(&q_lower))
                            || item
                                .item_id
                                .as_deref()
                                .is_some_and(|id| id.to_lowercase().contains(&q_lower))
                    })
                    .collect()
            } else {
                items
            };

            render(PinterestSearchResultsTemplate {
                total: items.len(),
                items,
                has_next,
                next_bookmark: page.bookmark,
                error: None,
            })
        }
        Err(e) => render(PinterestSearchResultsTemplate {
            items: vec![],
            total: 0,
            has_next: false,
            next_bookmark: None,
            error: Some(format_api_error(&e)),
        }),
    }
}

/// GET /pinterest/mappings -- Product mappings list page.
#[instrument(skip(state, session))]
async fn mappings_page(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let repo = PinterestProductMappingRepository::new(state.pool());
    let mappings = repo.list_all().await.unwrap_or_default();
    let mapping_count = mappings.len();

    let template = PinterestMappingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/pinterest/mappings".to_string(),
        mappings,
        mapping_count,
    };

    render(template)
}

/// POST /pinterest/mappings/create -- Create a product mapping (JSON API).
#[instrument(skip(state, session, req))]
async fn create_mapping(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<CreateMappingRequest>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    if req.pinterest_item_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Pinterest item ID is required")),
        )
            .into_response();
    }

    let repo = PinterestProductMappingRepository::new(state.pool());
    let params = CreatePinterestMappingParams {
        shopify_product_id: req.shopify_product_id.trim(),
        shopify_variant_id: req.shopify_variant_id.as_deref(),
        pinterest_item_id: req.pinterest_item_id.trim(),
        match_type: req.match_type.as_deref().unwrap_or("manual"),
    };

    match repo.create(&params).await {
        Ok(mapping) => {
            let template = PinterestMappingRowTemplate { mapping };
            render(template)
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to create Pinterest product mapping");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to create mapping")),
            )
                .into_response()
        }
    }
}

/// POST /pinterest/mappings/{id}/delete -- Delete a product mapping.
#[instrument(skip(state, session))]
async fn delete_mapping(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i32>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let repo = PinterestProductMappingRepository::new(state.pool());
    match repo.delete(id).await {
        Ok(true) => StatusCode::OK.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Mapping not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete Pinterest product mapping");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to delete mapping")),
            )
                .into_response()
        }
    }
}

/// POST /pinterest/mappings/auto-match -- Auto-match Shopify products to Pinterest catalog.
#[instrument(skip(state, session))]
async fn auto_match(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(pinterest) = state.pinterest() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Pinterest API not connected")),
        )
            .into_response();
    };

    let repo = PinterestProductMappingRepository::new(state.pool());

    match run_auto_match(state.shopify(), pinterest, &repo).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Auto-match failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(format!("Auto-match failed: {e}"))),
            )
                .into_response()
        }
    }
}

/// Result of an auto-match run.
#[derive(Debug, Serialize)]
struct AutoMatchResult {
    matched: usize,
    skipped: usize,
    failed: usize,
    message: String,
}

/// Run auto-match across all Shopify products against the Pinterest catalog.
async fn run_auto_match(
    shopify: &crate::shopify::AdminClient,
    _pinterest: &PinterestClient,
    repo: &PinterestProductMappingRepository<'_>,
) -> Result<AutoMatchResult, String> {
    // Pinterest uses product groups rather than a flat catalog search.
    // For auto-match, we iterate Shopify products and match by title.
    // In a full implementation, we'd fetch all catalog items from a feed first.
    let matched = 0usize;
    let mut skipped = 0usize;
    let mut cursor: Option<String> = None;

    loop {
        let page = shopify
            .get_products(50, cursor, None)
            .await
            .map_err(|e| format!("Shopify API error: {e}"))?;

        for product in &page.products {
            // Check if mapping already exists
            if repo
                .get_by_shopify_product(&product.id)
                .await
                .ok()
                .is_some_and(|mappings| !mappings.is_empty())
            {
                skipped += 1;
                continue;
            }

            // Without a flat catalog search, we skip unmatched products for now.
            // Users can create manual mappings via the catalog search page.
            skipped += 1;
        }

        if page.page_info.has_next_page {
            cursor = page.page_info.end_cursor;
        } else {
            break;
        }
    }

    let message = format!("Auto-match complete: {matched} matched, {skipped} skipped, 0 failed");
    tracing::info!(%message);
    Ok(AutoMatchResult {
        matched,
        skipped,
        failed: 0,
        message,
    })
}

// =============================================================================
// Helpers
// =============================================================================

/// Get the current admin from the session.
async fn get_admin(session: &Session) -> Option<CurrentAdmin> {
    session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
}

/// Render an Askama template into an HTML response.
fn render(template: impl Template) -> Response {
    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

/// Format a Pinterest API error into a user-friendly message.
fn format_api_error(error: &PinterestError) -> String {
    match error {
        PinterestError::Unauthorized(_) => {
            "Authentication failed. Please check your credentials in Settings.".to_string()
        }
        PinterestError::RateLimited(wait) => {
            format!("Rate limited by Pinterest. Please wait {wait} seconds and try again.")
        }
        PinterestError::NotFound(_) => "No results found.".to_string(),
        _ => format!("Search failed: {error}"),
    }
}

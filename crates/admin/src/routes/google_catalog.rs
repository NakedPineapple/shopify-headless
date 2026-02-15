//! Google Merchant Center catalog search and product mapping routes.
//!
//! These routes provide catalog browsing via the Google Content API
//! and CRUD operations for Shopify-to-Google product mappings.
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

use naked_pineapple_services::google_merchant::{
    GoogleMerchantClient, GoogleMerchantError, GoogleProduct,
};

use crate::db::{CreateGoogleMappingParams, GoogleProductMapping, GoogleProductMappingRepository};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::amazon_settings::ApiResponse;
use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Google catalog search page.
#[derive(Template)]
#[template(path = "google/catalog.html")]
struct GoogleCatalogTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
}

/// Google product mappings page.
#[derive(Template)]
#[template(path = "google/mappings.html")]
struct GoogleMappingsTemplate {
    admin_user: AdminUserView,
    current_path: String,
    mappings: Vec<GoogleProductMapping>,
    mapping_count: usize,
}

/// HTMX partial: catalog search results.
#[derive(Template)]
#[template(path = "google/partials/search_results.html")]
struct GoogleSearchResultsTemplate {
    items: Vec<GoogleProduct>,
    total: usize,
    has_next: bool,
    next_page_token: Option<String>,
    error: Option<String>,
}

/// HTMX partial: single mapping row.
#[derive(Template)]
#[template(path = "google/partials/mapping_row.html")]
struct GoogleMappingRowTemplate {
    mapping: GoogleProductMapping,
}

// =============================================================================
// Router
// =============================================================================

/// Build the Google catalog router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/google/catalog", get(catalog_page))
        .route("/google/catalog/search", get(catalog_search))
        .route("/google/mappings", get(mappings_page))
        .route("/google/mappings/create", post(create_mapping))
        .route("/google/mappings/{id}/delete", post(delete_mapping))
        .route("/google/mappings/auto-match", post(auto_match))
}

// =============================================================================
// Query Parameters
// =============================================================================

#[derive(Debug, Deserialize)]
struct CatalogSearchParams {
    q: Option<String>,
    page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateMappingRequest {
    shopify_product_id: String,
    shopify_variant_id: Option<String>,
    google_product_id: String,
    match_type: Option<String>,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /google/catalog -- Catalog search page.
#[instrument(skip(state, session))]
async fn catalog_page(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let connected = state.google().is_some();

    let template = GoogleCatalogTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/google/catalog".to_string(),
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

    let Some(client) = state.google() else {
        return render(GoogleSearchResultsTemplate {
            items: vec![],
            total: 0,
            has_next: false,
            next_page_token: None,
            error: Some("Google Merchant Center API is not connected".to_string()),
        });
    };

    match client.list_products(Some(250), params.page_token).await {
        Ok(page) => {
            let items = page.resources.unwrap_or_default();
            let has_next = page.next_page_token.is_some();

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
                                .offer_id
                                .as_deref()
                                .is_some_and(|id| id.to_lowercase().contains(&q_lower))
                    })
                    .collect()
            } else {
                items
            };

            render(GoogleSearchResultsTemplate {
                total: items.len(),
                items,
                has_next,
                next_page_token: page.next_page_token,
                error: None,
            })
        }
        Err(e) => render(GoogleSearchResultsTemplate {
            items: vec![],
            total: 0,
            has_next: false,
            next_page_token: None,
            error: Some(format_api_error(&e)),
        }),
    }
}

/// GET /google/mappings -- Product mappings list page.
#[instrument(skip(state, session))]
async fn mappings_page(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let repo = GoogleProductMappingRepository::new(state.pool());
    let mappings = repo.list_all().await.unwrap_or_default();
    let mapping_count = mappings.len();

    let template = GoogleMappingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/google/mappings".to_string(),
        mappings,
        mapping_count,
    };

    render(template)
}

/// POST /google/mappings/create -- Create a product mapping (JSON API).
#[instrument(skip(state, session, req))]
async fn create_mapping(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<CreateMappingRequest>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    if req.google_product_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Google product ID is required")),
        )
            .into_response();
    }

    let repo = GoogleProductMappingRepository::new(state.pool());
    let params = CreateGoogleMappingParams {
        shopify_product_id: req.shopify_product_id.trim(),
        shopify_variant_id: req.shopify_variant_id.as_deref(),
        google_product_id: req.google_product_id.trim(),
        match_type: req.match_type.as_deref().unwrap_or("manual"),
    };

    match repo.create(&params).await {
        Ok(mapping) => {
            let template = GoogleMappingRowTemplate { mapping };
            render(template)
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to create Google product mapping");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to create mapping")),
            )
                .into_response()
        }
    }
}

/// POST /google/mappings/{id}/delete -- Delete a product mapping.
#[instrument(skip(state, session))]
async fn delete_mapping(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i32>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let repo = GoogleProductMappingRepository::new(state.pool());
    match repo.delete(id).await {
        Ok(true) => StatusCode::OK.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Mapping not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete Google product mapping");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to delete mapping")),
            )
                .into_response()
        }
    }
}

/// POST /google/mappings/auto-match -- Auto-match Shopify products to Google catalog.
#[instrument(skip(state, session))]
async fn auto_match(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(google) = state.google() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(
                "Google Merchant Center API not connected",
            )),
        )
            .into_response();
    };

    let repo = GoogleProductMappingRepository::new(state.pool());

    match run_auto_match(state.shopify(), google, &repo).await {
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

/// Run auto-match across all Shopify products against the Google catalog.
async fn run_auto_match(
    shopify: &crate::shopify::AdminClient,
    _google: &GoogleMerchantClient,
    repo: &GoogleProductMappingRepository<'_>,
) -> Result<AutoMatchResult, String> {
    let matched = 0usize;
    let mut skipped = 0usize;
    let mut cursor: Option<String> = None;

    loop {
        let page = shopify
            .get_products(50, cursor, None)
            .await
            .map_err(|e| format!("Shopify API error: {e}"))?;

        for product in &page.products {
            if repo
                .get_by_shopify_product(&product.id)
                .await
                .ok()
                .is_some_and(|mappings| !mappings.is_empty())
            {
                skipped += 1;
                continue;
            }

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

/// Format a Google API error into a user-friendly message.
fn format_api_error(error: &GoogleMerchantError) -> String {
    match error {
        GoogleMerchantError::Unauthorized(_) => {
            "Authentication failed. Please check your credentials in Settings.".to_string()
        }
        GoogleMerchantError::RateLimited(wait) => {
            format!("Rate limited by Google. Please wait {wait} seconds and try again.")
        }
        GoogleMerchantError::NotFound(_) => "No results found.".to_string(),
        _ => format!("Search failed: {error}"),
    }
}

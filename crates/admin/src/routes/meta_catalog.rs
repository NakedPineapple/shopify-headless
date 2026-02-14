//! Meta Commerce catalog search and product mapping routes.
//!
//! These routes provide catalog search via the Meta Graph API (Facebook Shop
//! catalog) and CRUD operations for Shopify-to-Facebook product mappings.
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

use naked_pineapple_services::meta_commerce::{
    FacebookProduct, MetaCommerceClient, MetaCommerceError,
};

use crate::db::{CreateMetaMappingParams, MetaProductMapping, MetaProductMappingRepository};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::amazon_settings::ApiResponse;
use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Meta catalog search page.
#[derive(Template)]
#[template(path = "meta/catalog.html")]
struct MetaCatalogTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
}

/// Meta product mappings page.
#[derive(Template)]
#[template(path = "meta/mappings.html")]
struct MetaMappingsTemplate {
    admin_user: AdminUserView,
    current_path: String,
    mappings: Vec<MetaProductMapping>,
    mapping_count: usize,
}

/// HTMX partial: catalog search results.
#[derive(Template)]
#[template(path = "meta/partials/search_results.html")]
struct MetaSearchResultsTemplate {
    items: Vec<FacebookProduct>,
    total: usize,
    has_next: bool,
    next_cursor: Option<String>,
    catalog_id: String,
    error: Option<String>,
}

/// HTMX partial: single mapping row.
#[derive(Template)]
#[template(path = "meta/partials/mapping_row.html")]
struct MetaMappingRowTemplate {
    mapping: MetaProductMapping,
}

// =============================================================================
// Router
// =============================================================================

/// Build the Meta catalog router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/meta/catalog", get(catalog_page))
        .route("/meta/catalog/search", get(catalog_search))
        .route("/meta/mappings", get(mappings_page))
        .route("/meta/mappings/create", post(create_mapping))
        .route("/meta/mappings/{id}/delete", post(delete_mapping))
        .route("/meta/mappings/auto-match", post(auto_match))
}

// =============================================================================
// Query Parameters
// =============================================================================

#[derive(Debug, Deserialize)]
struct CatalogSearchParams {
    q: Option<String>,
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateMappingRequest {
    shopify_product_id: String,
    shopify_variant_id: Option<String>,
    facebook_product_id: String,
    retailer_id: Option<String>,
    match_type: Option<String>,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /meta/catalog -- Catalog search page.
#[instrument(skip(state, session))]
async fn catalog_page(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let connected = state.meta().is_some();

    let template = MetaCatalogTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/meta/catalog".to_string(),
        connected,
    };

    render(template)
}

/// GET /meta/catalog/search?q=... -- HTMX search results partial.
#[instrument(skip(state, session))]
async fn catalog_search(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<CatalogSearchParams>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(query) = params.q.as_deref().filter(|q| !q.trim().is_empty()) else {
        return render(MetaSearchResultsTemplate {
            items: vec![],
            total: 0,
            has_next: false,
            next_cursor: None,
            catalog_id: String::new(),
            error: None,
        });
    };

    let Some(client) = state.meta() else {
        return render(MetaSearchResultsTemplate {
            items: vec![],
            total: 0,
            has_next: false,
            next_cursor: None,
            catalog_id: String::new(),
            error: Some("Meta Commerce API is not connected".to_string()),
        });
    };

    let catalog_id = client.catalog_id().to_string();

    match client.search_products(query, 25, params.after).await {
        Ok(page) => {
            let next_cursor = page
                .paging
                .as_ref()
                .and_then(|p| p.cursors.as_ref())
                .and_then(|c| c.after.clone());
            let has_next = page.paging.as_ref().and_then(|p| p.next.as_ref()).is_some();
            let total = page.data.len();

            render(MetaSearchResultsTemplate {
                items: page.data,
                total,
                has_next,
                next_cursor,
                catalog_id,
                error: None,
            })
        }
        Err(e) => render(MetaSearchResultsTemplate {
            items: vec![],
            total: 0,
            has_next: false,
            next_cursor: None,
            catalog_id,
            error: Some(format_api_error(&e)),
        }),
    }
}

/// GET /meta/mappings -- Product mappings list page.
#[instrument(skip(state, session))]
async fn mappings_page(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let repo = MetaProductMappingRepository::new(state.pool());
    let mappings = repo.list_all().await.unwrap_or_default();
    let mapping_count = mappings.len();

    let template = MetaMappingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/meta/mappings".to_string(),
        mappings,
        mapping_count,
    };

    render(template)
}

/// POST /meta/mappings/create -- Create a product mapping (JSON API).
#[instrument(skip(state, session, req))]
async fn create_mapping(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<CreateMappingRequest>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    if req.facebook_product_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Facebook product ID is required")),
        )
            .into_response();
    }

    let repo = MetaProductMappingRepository::new(state.pool());
    let params = CreateMetaMappingParams {
        shopify_product_id: req.shopify_product_id.trim(),
        shopify_variant_id: req.shopify_variant_id.as_deref(),
        facebook_product_id: req.facebook_product_id.trim(),
        retailer_id: req.retailer_id.as_deref(),
        match_type: req.match_type.as_deref().unwrap_or("manual"),
    };

    match repo.create(&params).await {
        Ok(mapping) => {
            let template = MetaMappingRowTemplate { mapping };
            render(template)
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to create Meta product mapping");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to create mapping")),
            )
                .into_response()
        }
    }
}

/// POST /meta/mappings/{id}/delete -- Delete a product mapping.
#[instrument(skip(state, session))]
async fn delete_mapping(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i32>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let repo = MetaProductMappingRepository::new(state.pool());
    match repo.delete(id).await {
        Ok(true) => StatusCode::OK.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Mapping not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete Meta product mapping");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to delete mapping")),
            )
                .into_response()
        }
    }
}

/// POST /meta/mappings/auto-match -- Auto-match Shopify products to Meta catalog.
#[instrument(skip(state, session))]
async fn auto_match(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(meta) = state.meta() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Meta Commerce API not connected")),
        )
            .into_response();
    };

    let repo = MetaProductMappingRepository::new(state.pool());

    match run_auto_match(state.shopify(), meta, &repo).await {
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

/// Run auto-match across all Shopify products against the Meta catalog.
async fn run_auto_match(
    shopify: &crate::shopify::AdminClient,
    meta: &MetaCommerceClient,
    repo: &MetaProductMappingRepository<'_>,
) -> Result<AutoMatchResult, String> {
    // Fetch all Meta catalog products for matching
    let catalog_products = fetch_all_catalog_products(meta).await?;

    let mut matched = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    let mut cursor: Option<String> = None;

    loop {
        let page = shopify
            .get_products(50, cursor, None)
            .await
            .map_err(|e| format!("Shopify API error: {e}"))?;

        for product in &page.products {
            let result = try_match_product(repo, &catalog_products, product).await;
            match result {
                MatchOutcome::Matched(count) => matched += count,
                MatchOutcome::Skipped => skipped += 1,
                MatchOutcome::Failed => failed += 1,
            }
        }

        if page.page_info.has_next_page {
            cursor = page.page_info.end_cursor;
        } else {
            break;
        }
    }

    let message =
        format!("Auto-match complete: {matched} matched, {skipped} skipped, {failed} failed");
    tracing::info!(%message);
    Ok(AutoMatchResult {
        matched,
        skipped,
        failed,
        message,
    })
}

/// Fetch all products from the Meta catalog (paginated).
async fn fetch_all_catalog_products(
    meta: &MetaCommerceClient,
) -> Result<Vec<FacebookProduct>, String> {
    let mut all_products = Vec::new();
    let mut after: Option<String> = None;

    loop {
        let page = meta
            .search_products("", 100, after)
            .await
            .map_err(|e| format!("Meta catalog API error: {e}"))?;

        all_products.extend(page.data);

        let next = page
            .paging
            .as_ref()
            .and_then(|p| p.cursors.as_ref())
            .and_then(|c| c.after.clone());

        let has_next = page.paging.as_ref().and_then(|p| p.next.as_ref()).is_some();

        if has_next && next.is_some() {
            after = next;
        } else {
            break;
        }
    }

    Ok(all_products)
}

enum MatchOutcome {
    Matched(usize),
    Skipped,
    Failed,
}

/// Try to match a Shopify product to a Meta catalog product by `retailer_id` or title.
async fn try_match_product(
    repo: &MetaProductMappingRepository<'_>,
    catalog_products: &[FacebookProduct],
    product: &crate::shopify::types::AdminProduct,
) -> MatchOutcome {
    // Check if a mapping already exists for this Shopify product
    if repo
        .get_by_shopify_product(&product.id)
        .await
        .ok()
        .is_some_and(|mappings| !mappings.is_empty())
    {
        return MatchOutcome::Skipped;
    }

    // Try to match by retailer_id first (exact SKU match)
    if let Some(fb_product) = match_by_retailer_id(product, catalog_products) {
        return create_match(repo, product, fb_product, "retailer_id").await;
    }

    // Fall back to matching by title (case-insensitive)
    if let Some(fb_product) = match_by_title(product, catalog_products) {
        return create_match(repo, product, fb_product, "title").await;
    }

    MatchOutcome::Skipped
}

/// Match a Shopify product to a Meta catalog product by `retailer_id` (SKU).
fn match_by_retailer_id<'a>(
    product: &crate::shopify::types::AdminProduct,
    catalog_products: &'a [FacebookProduct],
) -> Option<&'a FacebookProduct> {
    for variant in &product.variants {
        let sku = variant.sku.as_deref().filter(|s| !s.trim().is_empty())?;
        let found = catalog_products
            .iter()
            .find(|fb| fb.retailer_id.as_deref().is_some_and(|rid| rid == sku));
        if found.is_some() {
            return found;
        }
    }
    None
}

/// Match a Shopify product to a Meta catalog product by title (case-insensitive).
fn match_by_title<'a>(
    product: &crate::shopify::types::AdminProduct,
    catalog_products: &'a [FacebookProduct],
) -> Option<&'a FacebookProduct> {
    let shopify_title = product.title.to_lowercase();
    catalog_products.iter().find(|fb| {
        fb.name
            .as_deref()
            .is_some_and(|name| name.to_lowercase() == shopify_title)
    })
}

/// Create a mapping from a matched Shopify product and Facebook product.
async fn create_match(
    repo: &MetaProductMappingRepository<'_>,
    product: &crate::shopify::types::AdminProduct,
    fb_product: &FacebookProduct,
    match_type: &str,
) -> MatchOutcome {
    let params = CreateMetaMappingParams {
        shopify_product_id: &product.id,
        shopify_variant_id: None,
        facebook_product_id: &fb_product.id,
        retailer_id: fb_product.retailer_id.as_deref(),
        match_type,
    };

    match repo.create(&params).await {
        Ok(_) => MatchOutcome::Matched(1),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to create auto-match mapping");
            MatchOutcome::Failed
        }
    }
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

/// Format a Meta Commerce API error into a user-friendly message.
fn format_api_error(error: &MetaCommerceError) -> String {
    match error {
        MetaCommerceError::Unauthorized(_) => {
            "Authentication failed. Please check your credentials in Settings.".to_string()
        }
        MetaCommerceError::RateLimited(wait) => {
            format!("Rate limited by Meta. Please wait {wait} seconds and try again.")
        }
        MetaCommerceError::NotFound(_) => "No results found.".to_string(),
        _ => format!("Search failed: {error}"),
    }
}

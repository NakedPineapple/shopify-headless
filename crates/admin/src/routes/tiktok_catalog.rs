//! TikTok Shop catalog search and product mapping routes.
//!
//! These routes provide catalog search via the TikTok Shop API
//! and CRUD operations for Shopify-to-TikTok product mappings.
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

use naked_pineapple_services::tiktok_shop::{TikTokProduct, TikTokShopClient, TikTokShopError};

use crate::db::{CreateTikTokMappingParams, TikTokProductMapping, TikTokProductMappingRepository};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::amazon_settings::ApiResponse;
use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// TikTok catalog search page.
#[derive(Template)]
#[template(path = "tiktok/catalog.html")]
struct TikTokCatalogTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
}

/// TikTok product mappings page.
#[derive(Template)]
#[template(path = "tiktok/mappings.html")]
struct TikTokMappingsTemplate {
    admin_user: AdminUserView,
    current_path: String,
    mappings: Vec<TikTokProductMapping>,
    mapping_count: usize,
}

/// HTMX partial: catalog search results.
#[derive(Template)]
#[template(path = "tiktok/partials/search_results.html")]
struct TikTokSearchResultsTemplate {
    items: Vec<TikTokProduct>,
    total: usize,
    has_next: bool,
    next_page_token: Option<String>,
    error: Option<String>,
}

/// HTMX partial: single mapping row.
#[derive(Template)]
#[template(path = "tiktok/partials/mapping_row.html")]
struct TikTokMappingRowTemplate {
    mapping: TikTokProductMapping,
}

// =============================================================================
// Router
// =============================================================================

/// Build the TikTok catalog router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tiktok/catalog", get(catalog_page))
        .route("/tiktok/catalog/search", get(catalog_search))
        .route("/tiktok/mappings", get(mappings_page))
        .route("/tiktok/mappings/create", post(create_mapping))
        .route("/tiktok/mappings/{id}/delete", post(delete_mapping))
        .route("/tiktok/mappings/auto-match", post(auto_match))
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
    tiktok_product_id: String,
    tiktok_sku_id: Option<String>,
    match_type: Option<String>,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /tiktok/catalog -- Catalog search page.
#[instrument(skip(state, session))]
async fn catalog_page(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let connected = state.tiktok().is_some();

    let template = TikTokCatalogTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/tiktok/catalog".to_string(),
        connected,
    };

    render(template)
}

/// GET /tiktok/catalog/search?q=... -- HTMX search results partial.
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
        return render(TikTokSearchResultsTemplate {
            items: vec![],
            total: 0,
            has_next: false,
            next_page_token: None,
            error: None,
        });
    };

    let Some(client) = state.tiktok() else {
        return render(TikTokSearchResultsTemplate {
            items: vec![],
            total: 0,
            has_next: false,
            next_page_token: None,
            error: Some("TikTok Shop API is not connected".to_string()),
        });
    };

    match client
        .search_products(query, 25, params.after.as_deref())
        .await
    {
        Ok(data) => {
            let products = data.products.unwrap_or_default();
            let total = products.len();
            let has_next = data.next_page_token.as_ref().is_some_and(|t| !t.is_empty());

            render(TikTokSearchResultsTemplate {
                items: products,
                total,
                has_next,
                next_page_token: data.next_page_token,
                error: None,
            })
        }
        Err(e) => render(TikTokSearchResultsTemplate {
            items: vec![],
            total: 0,
            has_next: false,
            next_page_token: None,
            error: Some(format_api_error(&e)),
        }),
    }
}

/// GET /tiktok/mappings -- Product mappings list page.
#[instrument(skip(state, session))]
async fn mappings_page(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let repo = TikTokProductMappingRepository::new(state.pool());
    let mappings = repo.list_all().await.unwrap_or_default();
    let mapping_count = mappings.len();

    let template = TikTokMappingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/tiktok/mappings".to_string(),
        mappings,
        mapping_count,
    };

    render(template)
}

/// POST /tiktok/mappings/create -- Create a product mapping (JSON API).
#[instrument(skip(state, session, req))]
async fn create_mapping(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<CreateMappingRequest>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    if req.tiktok_product_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("TikTok product ID is required")),
        )
            .into_response();
    }

    let repo = TikTokProductMappingRepository::new(state.pool());
    let params = CreateTikTokMappingParams {
        shopify_product_id: req.shopify_product_id.trim(),
        shopify_variant_id: req.shopify_variant_id.as_deref(),
        tiktok_product_id: req.tiktok_product_id.trim(),
        tiktok_sku_id: req.tiktok_sku_id.as_deref(),
        match_type: req.match_type.as_deref().unwrap_or("manual"),
    };

    match repo.create(&params).await {
        Ok(mapping) => {
            let template = TikTokMappingRowTemplate { mapping };
            render(template)
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to create TikTok product mapping");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to create mapping")),
            )
                .into_response()
        }
    }
}

/// POST /tiktok/mappings/{id}/delete -- Delete a product mapping.
#[instrument(skip(state, session))]
async fn delete_mapping(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i32>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let repo = TikTokProductMappingRepository::new(state.pool());
    match repo.delete(id).await {
        Ok(true) => StatusCode::OK.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Mapping not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete TikTok product mapping");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to delete mapping")),
            )
                .into_response()
        }
    }
}

/// POST /tiktok/mappings/auto-match -- Auto-match Shopify products to TikTok catalog.
#[instrument(skip(state, session))]
async fn auto_match(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(tiktok) = state.tiktok() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("TikTok Shop API not connected")),
        )
            .into_response();
    };

    let repo = TikTokProductMappingRepository::new(state.pool());

    match run_auto_match(state.shopify(), tiktok, &repo).await {
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

/// Run auto-match across all Shopify products against the TikTok catalog.
async fn run_auto_match(
    shopify: &crate::shopify::AdminClient,
    tiktok: &TikTokShopClient,
    repo: &TikTokProductMappingRepository<'_>,
) -> Result<AutoMatchResult, String> {
    let catalog_products = fetch_all_catalog_products(tiktok).await?;

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

/// Fetch all products from the TikTok Shop catalog (paginated).
async fn fetch_all_catalog_products(
    tiktok: &TikTokShopClient,
) -> Result<Vec<TikTokProduct>, String> {
    let mut all_products = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let data = tiktok
            .search_products("", 100, page_token.as_deref())
            .await
            .map_err(|e| format!("TikTok catalog API error: {e}"))?;

        if let Some(products) = data.products {
            all_products.extend(products);
        }

        let next = data.next_page_token.filter(|t| !t.is_empty());

        if let Some(token) = next {
            page_token = Some(token);
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

/// Try to match a Shopify product to a TikTok catalog product by SKU or title.
async fn try_match_product(
    repo: &TikTokProductMappingRepository<'_>,
    catalog_products: &[TikTokProduct],
    product: &crate::shopify::types::AdminProduct,
) -> MatchOutcome {
    if repo
        .get_by_shopify_product(&product.id)
        .await
        .ok()
        .is_some_and(|mappings| !mappings.is_empty())
    {
        return MatchOutcome::Skipped;
    }

    if let Some(tt_product) = match_by_sku(product, catalog_products) {
        return create_match(repo, product, tt_product, "sku").await;
    }

    if let Some(tt_product) = match_by_title(product, catalog_products) {
        return create_match(repo, product, tt_product, "title").await;
    }

    MatchOutcome::Skipped
}

/// Match a Shopify product to a TikTok catalog product by SKU.
fn match_by_sku<'a>(
    product: &crate::shopify::types::AdminProduct,
    catalog_products: &'a [TikTokProduct],
) -> Option<&'a TikTokProduct> {
    for variant in &product.variants {
        let sku = variant.sku.as_deref().filter(|s| !s.trim().is_empty())?;
        let found = catalog_products.iter().find(|tp| {
            tp.skus
                .as_ref()
                .is_some_and(|skus| skus.iter().any(|s| s.seller_sku.as_deref() == Some(sku)))
        });
        if found.is_some() {
            return found;
        }
    }
    None
}

/// Match a Shopify product to a TikTok catalog product by title (case-insensitive).
fn match_by_title<'a>(
    product: &crate::shopify::types::AdminProduct,
    catalog_products: &'a [TikTokProduct],
) -> Option<&'a TikTokProduct> {
    let shopify_title = product.title.to_lowercase();
    catalog_products.iter().find(|tp| {
        tp.title
            .as_deref()
            .is_some_and(|title| title.to_lowercase() == shopify_title)
    })
}

/// Create a mapping from a matched Shopify product and TikTok product.
async fn create_match(
    repo: &TikTokProductMappingRepository<'_>,
    product: &crate::shopify::types::AdminProduct,
    tt_product: &TikTokProduct,
    match_type: &str,
) -> MatchOutcome {
    let tiktok_product_id = tt_product.id.as_deref().unwrap_or("unknown");
    let tiktok_sku_id = tt_product
        .skus
        .as_ref()
        .and_then(|skus| skus.first())
        .and_then(|s| s.id.as_deref());

    let params = CreateTikTokMappingParams {
        shopify_product_id: &product.id,
        shopify_variant_id: None,
        tiktok_product_id,
        tiktok_sku_id,
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

/// Format a TikTok Shop API error into a user-friendly message.
fn format_api_error(error: &TikTokShopError) -> String {
    match error {
        TikTokShopError::Unauthorized(_) => {
            "Authentication failed. Please check your credentials in Settings.".to_string()
        }
        TikTokShopError::RateLimited(wait) => {
            format!("Rate limited by TikTok. Please wait {wait} seconds and try again.")
        }
        TikTokShopError::NotFound(_) => "No results found.".to_string(),
        _ => format!("Search failed: {error}"),
    }
}

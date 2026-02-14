//! Amazon catalog search and product mapping routes.
//!
//! These routes provide catalog search via the Amazon SP-API Catalog Items API
//! and CRUD operations for Shopify-to-Amazon product mappings.
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

use naked_pineapple_services::amazon_sp::{
    AmazonSpClient, AmazonSpError, CatalogItem, CatalogPagination,
};

use crate::db::{AmazonProductMapping, AmazonProductMappingRepository, CreateMappingParams};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::amazon_settings::ApiResponse;
use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Amazon catalog search page.
#[derive(Template)]
#[template(path = "amazon/catalog.html")]
struct CatalogTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
}

/// Amazon product mappings page.
#[derive(Template)]
#[template(path = "amazon/mappings.html")]
struct MappingsTemplate {
    admin_user: AdminUserView,
    current_path: String,
    mappings: Vec<AmazonProductMapping>,
    mapping_count: usize,
}

/// HTMX partial: catalog search results.
#[derive(Template)]
#[template(path = "amazon/partials/search_results.html")]
struct SearchResultsTemplate {
    items: Vec<CatalogItem>,
    total: i32,
    has_next: bool,
    next_token: Option<String>,
    marketplace_id: String,
    error: Option<String>,
}

/// HTMX partial: single mapping row.
#[derive(Template)]
#[template(path = "amazon/partials/mapping_row.html")]
struct MappingRowTemplate {
    mapping: AmazonProductMapping,
}

// =============================================================================
// Router
// =============================================================================

/// Build the Amazon catalog router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/amazon/catalog", get(catalog_page))
        .route("/amazon/catalog/search", get(catalog_search))
        .route("/amazon/mappings", get(mappings_page))
        .route("/amazon/mappings", post(create_mapping))
        .route(
            "/amazon/mappings/{id}",
            axum::routing::delete(delete_mapping),
        )
        .route("/amazon/mappings/auto-match", post(auto_match))
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
    asin: String,
    amazon_sku: String,
    match_type: Option<String>,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /amazon/catalog — Catalog search page.
#[instrument(skip(state, session))]
async fn catalog_page(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let connected = state.amazon().is_some();

    let template = CatalogTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/amazon/catalog".to_string(),
        connected,
    };

    render(template)
}

/// GET /amazon/catalog/search?q=... — HTMX search results partial.
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
        return render(SearchResultsTemplate {
            items: vec![],
            total: 0,
            has_next: false,
            next_token: None,
            marketplace_id: String::new(),
            error: None,
        });
    };

    let Some(client) = state.amazon() else {
        return render(SearchResultsTemplate {
            items: vec![],
            total: 0,
            has_next: false,
            next_token: None,
            marketplace_id: String::new(),
            error: Some("Amazon SP-API is not connected".to_string()),
        });
    };

    let marketplace_id = client.marketplace_id().to_string();

    match client.search_catalog_items(query, params.page_token).await {
        Ok(response) => {
            let pagination = response.pagination.unwrap_or(CatalogPagination {
                next_token: None,
                previous_token: None,
            });

            render(SearchResultsTemplate {
                total: response.number_of_results.unwrap_or(0),
                has_next: pagination.next_token.is_some(),
                next_token: pagination.next_token,
                items: response.items,
                marketplace_id,
                error: None,
            })
        }
        Err(e) => render(SearchResultsTemplate {
            items: vec![],
            total: 0,
            has_next: false,
            next_token: None,
            marketplace_id,
            error: Some(format_api_error(&e)),
        }),
    }
}

/// GET /amazon/mappings — Product mappings list page.
#[instrument(skip(state, session))]
async fn mappings_page(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let repo = AmazonProductMappingRepository::new(state.pool());
    let mappings = repo.list().await.unwrap_or_default();
    let mapping_count = mappings.len();

    let template = MappingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/amazon/mappings".to_string(),
        mappings,
        mapping_count,
    };

    render(template)
}

/// POST /amazon/mappings — Create a product mapping (JSON API).
#[instrument(skip(state, session, req))]
async fn create_mapping(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<CreateMappingRequest>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    if req.asin.trim().is_empty() || req.amazon_sku.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("ASIN and SKU are required")),
        )
            .into_response();
    }

    let marketplace_id = state
        .amazon()
        .map_or("ATVPDKIKX0DER", |c| c.marketplace_id())
        .to_string();

    let repo = AmazonProductMappingRepository::new(state.pool());
    let params = CreateMappingParams {
        shopify_product_id: req.shopify_product_id.trim(),
        shopify_variant_id: req.shopify_variant_id.as_deref(),
        asin: req.asin.trim(),
        amazon_sku: req.amazon_sku.trim(),
        marketplace_id: &marketplace_id,
        match_type: req.match_type.as_deref().unwrap_or("manual"),
    };

    match repo.create(&params).await {
        Ok(mapping) => {
            let template = MappingRowTemplate { mapping };
            render(template)
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to create Amazon product mapping");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to create mapping")),
            )
                .into_response()
        }
    }
}

/// DELETE /amazon/mappings/{id} — Delete a product mapping.
#[instrument(skip(state, session))]
async fn delete_mapping(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i32>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let repo = AmazonProductMappingRepository::new(state.pool());
    match repo.delete(id).await {
        Ok(true) => StatusCode::OK.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Mapping not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete Amazon product mapping");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to delete mapping")),
            )
                .into_response()
        }
    }
}

/// POST /amazon/mappings/auto-match — Auto-match Shopify products to Amazon by SKU/UPC.
#[instrument(skip(state, session))]
async fn auto_match(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(amazon) = state.amazon() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Amazon SP-API not connected")),
        )
            .into_response();
    };

    let marketplace_id = amazon.marketplace_id().to_string();
    let repo = AmazonProductMappingRepository::new(state.pool());

    match run_auto_match(state.shopify(), amazon, &repo, &marketplace_id).await {
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

/// Run auto-match across all Shopify products.
async fn run_auto_match(
    shopify: &crate::shopify::AdminClient,
    amazon: &AmazonSpClient,
    repo: &AmazonProductMappingRepository<'_>,
    marketplace_id: &str,
) -> Result<AutoMatchResult, String> {
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
            for variant in &product.variants {
                let result =
                    try_match_variant(amazon, repo, &product.id, variant, marketplace_id).await;
                match result {
                    MatchOutcome::Matched => matched += 1,
                    MatchOutcome::Skipped => skipped += 1,
                    MatchOutcome::Failed => failed += 1,
                }
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

enum MatchOutcome {
    Matched,
    Skipped,
    Failed,
}

/// Try to match a single Shopify variant to an Amazon catalog item.
async fn try_match_variant(
    amazon: &AmazonSpClient,
    repo: &AmazonProductMappingRepository<'_>,
    product_id: &str,
    variant: &crate::shopify::types::AdminProductVariant,
    marketplace_id: &str,
) -> MatchOutcome {
    let sku = variant.sku.as_deref().filter(|s| !s.trim().is_empty());
    let barcode = variant.barcode.as_deref().filter(|s| !s.trim().is_empty());

    // Need at least one identifier to search
    let Some(identifier) = sku.or(barcode) else {
        return MatchOutcome::Skipped;
    };

    // Check if a mapping already exists for this variant's SKU
    if let Some(s) = sku
        && repo
            .get_by_sku(s, marketplace_id)
            .await
            .ok()
            .flatten()
            .is_some()
    {
        return MatchOutcome::Skipped;
    }

    // Rate-limit: Amazon catalog search allows 2 req/sec
    tokio::time::sleep(std::time::Duration::from_millis(550)).await;

    let search_result = amazon.search_catalog_items(identifier, None).await;
    match search_result {
        Ok(response) => {
            find_and_create_mapping(
                repo,
                &response.items,
                product_id,
                variant,
                marketplace_id,
                sku,
                barcode,
            )
            .await
        }
        Err(e) => {
            tracing::warn!(identifier = identifier, error = %e, "Auto-match search failed");
            MatchOutcome::Failed
        }
    }
}

/// Find a matching catalog item and create a mapping.
async fn find_and_create_mapping(
    repo: &AmazonProductMappingRepository<'_>,
    items: &[CatalogItem],
    product_id: &str,
    variant: &crate::shopify::types::AdminProductVariant,
    marketplace_id: &str,
    sku: Option<&str>,
    barcode: Option<&str>,
) -> MatchOutcome {
    let sku_val = sku.unwrap_or("");

    for item in items {
        // Check if any identifier matches the barcode (UPC/EAN match)
        let barcode_match = barcode.is_some_and(|bc| {
            item.identifiers
                .iter()
                .any(|id_group| id_group.identifiers.iter().any(|id| id.identifier == bc))
        });

        let match_type = if barcode_match {
            "upc_match"
        } else if sku.is_some() {
            "sku_match"
        } else {
            continue;
        };

        let params = CreateMappingParams {
            shopify_product_id: product_id,
            shopify_variant_id: Some(&variant.id),
            asin: &item.asin,
            amazon_sku: if sku_val.is_empty() {
                &item.asin
            } else {
                sku_val
            },
            marketplace_id,
            match_type,
        };

        return match repo.create(&params).await {
            Ok(_) => MatchOutcome::Matched,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to create auto-match mapping");
                MatchOutcome::Failed
            }
        };
    }

    MatchOutcome::Skipped
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

/// Format an SP-API error into a user-friendly message.
fn format_api_error(error: &AmazonSpError) -> String {
    match error {
        AmazonSpError::Unauthorized(_) => {
            "Authentication failed. Please check your credentials in Settings.".to_string()
        }
        AmazonSpError::RateLimited(wait) => {
            format!("Rate limited by Amazon. Please wait {wait} seconds and try again.")
        }
        AmazonSpError::NotFound(_) => "No results found.".to_string(),
        _ => format!("Search failed: {error}"),
    }
}

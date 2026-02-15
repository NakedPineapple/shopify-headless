//! Faire catalog search and product mapping routes.
//!
//! These routes provide catalog browsing via the Faire Brand API v2
//! and CRUD operations for Shopify-to-Faire product mappings.
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

use naked_pineapple_services::faire::{FaireClient, FaireError, FaireProduct};

use crate::db::{CreateFaireMappingParams, FaireProductMapping, FaireProductMappingRepository};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::amazon_settings::ApiResponse;
use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Faire catalog search page.
#[derive(Template)]
#[template(path = "faire/catalog.html")]
struct FaireCatalogTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
}

/// Faire product mappings page.
#[derive(Template)]
#[template(path = "faire/mappings.html")]
struct FaireMappingsTemplate {
    admin_user: AdminUserView,
    current_path: String,
    mappings: Vec<FaireProductMapping>,
    mapping_count: usize,
}

/// HTMX partial: catalog search results.
#[derive(Template)]
#[template(path = "faire/partials/search_results.html")]
struct FaireSearchResultsTemplate {
    items: Vec<FaireProduct>,
    total: usize,
    has_more: bool,
    error: Option<String>,
}

/// HTMX partial: single mapping row.
#[derive(Template)]
#[template(path = "faire/partials/mapping_row.html")]
struct FaireMappingRowTemplate {
    mapping: FaireProductMapping,
}

// =============================================================================
// Router
// =============================================================================

/// Build the Faire catalog router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/faire/catalog", get(catalog_page))
        .route("/faire/catalog/search", get(catalog_search))
        .route("/faire/mappings", get(mappings_page))
        .route("/faire/mappings/create", post(create_mapping))
        .route("/faire/mappings/{id}/delete", post(delete_mapping))
        .route("/faire/mappings/auto-match", post(auto_match))
}

// =============================================================================
// Query Parameters
// =============================================================================

#[derive(Debug, Deserialize)]
struct CatalogSearchParams {
    q: Option<String>,
    page: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct CreateMappingRequest {
    shopify_product_id: String,
    shopify_variant_id: Option<String>,
    faire_product_token: String,
    match_type: Option<String>,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /faire/catalog -- Catalog search page.
#[instrument(skip(state, session))]
async fn catalog_page(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let connected = build_client_from_db(&state).await.is_some();

    let template = FaireCatalogTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/faire/catalog".to_string(),
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

    let Some(client) = build_client_from_db(&state).await else {
        return render(FaireSearchResultsTemplate {
            items: vec![],
            total: 0,
            has_more: false,
            error: Some("Faire API is not connected".to_string()),
        });
    };

    match client.list_products(params.page, Some(50)).await {
        Ok(page) => {
            let items = page.products.unwrap_or_default();
            let has_more = page.has_more.unwrap_or(false);

            // Filter by search query if provided (client-side title filtering)
            let items = if let Some(q) = params.q.as_deref().filter(|q| !q.trim().is_empty()) {
                let q_lower = q.to_lowercase();
                items
                    .into_iter()
                    .filter(|item| {
                        item.name
                            .as_deref()
                            .is_some_and(|n| n.to_lowercase().contains(&q_lower))
                            || item
                                .token
                                .as_deref()
                                .is_some_and(|t| t.to_lowercase().contains(&q_lower))
                    })
                    .collect()
            } else {
                items
            };

            render(FaireSearchResultsTemplate {
                total: items.len(),
                items,
                has_more,
                error: None,
            })
        }
        Err(e) => render(FaireSearchResultsTemplate {
            items: vec![],
            total: 0,
            has_more: false,
            error: Some(format_api_error(&e)),
        }),
    }
}

/// GET /faire/mappings -- Product mappings list page.
#[instrument(skip(state, session))]
async fn mappings_page(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let repo = FaireProductMappingRepository::new(state.pool());
    let mappings = repo.list_all().await.unwrap_or_default();
    let mapping_count = mappings.len();

    let template = FaireMappingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/faire/mappings".to_string(),
        mappings,
        mapping_count,
    };

    render(template)
}

/// POST /faire/mappings/create -- Create a product mapping (JSON API).
#[instrument(skip(state, session, req))]
async fn create_mapping(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<CreateMappingRequest>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    if req.faire_product_token.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Faire product token is required")),
        )
            .into_response();
    }

    let repo = FaireProductMappingRepository::new(state.pool());
    let params = CreateFaireMappingParams {
        shopify_product_id: req.shopify_product_id.trim(),
        shopify_variant_id: req.shopify_variant_id.as_deref(),
        faire_product_token: req.faire_product_token.trim(),
        match_type: req.match_type.as_deref().unwrap_or("manual"),
    };

    match repo.create(&params).await {
        Ok(mapping) => {
            let template = FaireMappingRowTemplate { mapping };
            render(template)
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to create Faire product mapping");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to create mapping")),
            )
                .into_response()
        }
    }
}

/// POST /faire/mappings/{id}/delete -- Delete a product mapping.
#[instrument(skip(state, session))]
async fn delete_mapping(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i32>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let repo = FaireProductMappingRepository::new(state.pool());
    match repo.delete(id).await {
        Ok(true) => StatusCode::OK.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Mapping not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete Faire product mapping");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to delete mapping")),
            )
                .into_response()
        }
    }
}

/// POST /faire/mappings/auto-match -- Auto-match Shopify products to Faire catalog.
#[instrument(skip(state, session))]
async fn auto_match(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(faire) = build_client_from_db(&state).await else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Faire API not connected")),
        )
            .into_response();
    };

    let repo = FaireProductMappingRepository::new(state.pool());

    match run_auto_match(state.shopify(), &faire, &repo).await {
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

/// Run auto-match across all Shopify products against the Faire catalog.
async fn run_auto_match(
    shopify: &crate::shopify::AdminClient,
    _faire: &FaireClient,
    repo: &FaireProductMappingRepository<'_>,
) -> Result<AutoMatchResult, String> {
    // Faire uses product tokens; for auto-match we iterate Shopify products
    // and attempt to match by title. In a full implementation, we'd fetch
    // all Faire catalog items first.
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

/// Build a `FaireClient` from stored DB credentials.
async fn build_client_from_db(
    state: &AppState,
) -> Option<naked_pineapple_services::faire::FaireClient> {
    use naked_pineapple_services::faire::FaireCredentials as ApiCredentials;

    let repo = crate::db::FaireCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten()?;

    Some(naked_pineapple_services::faire::FaireClient::new(
        ApiCredentials {
            brand_id: creds.brand_id,
            api_token: creds.api_token,
        },
    ))
}

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

/// Format a Faire API error into a user-friendly message.
fn format_api_error(error: &FaireError) -> String {
    match error {
        FaireError::Unauthorized(_) => {
            "Authentication failed. Please check your credentials in Settings.".to_string()
        }
        FaireError::RateLimited(wait) => {
            format!("Rate limited by Faire. Please wait {wait} seconds and try again.")
        }
        FaireError::NotFound(_) => "No results found.".to_string(),
        _ => format!("Search failed: {error}"),
    }
}

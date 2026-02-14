//! Amazon FBA inventory routes.
//!
//! Provides a unified cross-channel inventory view that combines data from
//! Shopify, `ShipHero` (warehouse), and Amazon FBA. Only `super_admin` users
//! can access these features.

use std::collections::HashMap;

use askama::Template;
use axum::{
    Router,
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use serde::Serialize;
use tower_sessions::Session;
use tracing::instrument;

use naked_pineapple_services::amazon_sp::InventorySummary;

use crate::db::AmazonProductMappingRepository;
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Amazon FBA inventory page.
#[derive(Template)]
#[template(path = "amazon/inventory.html")]
struct InventoryTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
    items: Vec<UnifiedInventoryRow>,
    fba_only: Vec<InventorySummary>,
    total_fba_units: i32,
    mapped_count: usize,
    error: Option<String>,
}

// =============================================================================
// View Models
// =============================================================================

/// A row in the unified inventory table.
#[derive(Debug, Clone, Serialize)]
pub struct UnifiedInventoryRow {
    /// Product name (from Amazon or mapping).
    pub product_name: String,
    /// Amazon SKU.
    pub amazon_sku: String,
    /// Amazon ASIN.
    pub asin: String,
    /// Shopify product ID (from mapping).
    pub shopify_product_id: Option<String>,
    /// Shopify inventory quantity (across all locations).
    pub shopify_stock: Option<i64>,
    /// `ShipHero` warehouse on-hand quantity.
    pub shiphero_stock: Option<i64>,
    /// FBA fulfillable quantity.
    pub fba_fulfillable: i32,
    /// FBA inbound quantity (receiving + working + shipped).
    pub fba_inbound: i32,
    /// FBA reserved quantity.
    pub fba_reserved: i32,
    /// FBA unfulfillable quantity.
    pub fba_unfulfillable: i32,
    /// FBA total quantity.
    pub fba_total: i32,
}

// =============================================================================
// Router
// =============================================================================

/// Build the Amazon inventory router.
pub fn router() -> Router<AppState> {
    Router::new().route("/amazon/inventory", get(inventory_page))
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /amazon/inventory — Unified FBA inventory view.
#[instrument(skip(state, session))]
async fn inventory_page(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let connected = state.amazon().is_some();
    if !connected {
        return render(InventoryTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/amazon/inventory".to_string(),
            connected: false,
            items: vec![],
            fba_only: vec![],
            total_fba_units: 0,
            mapped_count: 0,
            error: None,
        });
    }

    let result = build_inventory_view(&state).await;
    match result {
        Ok(view) => render(InventoryTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/amazon/inventory".to_string(),
            connected: true,
            items: view.items,
            fba_only: view.fba_only,
            total_fba_units: view.total_fba_units,
            mapped_count: view.mapped_count,
            error: None,
        }),
        Err(e) => render(InventoryTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/amazon/inventory".to_string(),
            connected: true,
            items: vec![],
            fba_only: vec![],
            total_fba_units: 0,
            mapped_count: 0,
            error: Some(e),
        }),
    }
}

// =============================================================================
// Inventory Aggregation
// =============================================================================

struct InventoryView {
    items: Vec<UnifiedInventoryRow>,
    fba_only: Vec<InventorySummary>,
    total_fba_units: i32,
    mapped_count: usize,
}

/// Build the unified inventory view by cross-referencing FBA data with mappings.
async fn build_inventory_view(state: &AppState) -> Result<InventoryView, String> {
    let client = state.amazon().ok_or("Amazon SP-API not connected")?;

    // Use moka cache for FBA summaries (5-min TTL)
    let cache_key = "fba_inventory".to_string();
    let summaries = if let Some(cached) = state.fba_cache().get(&cache_key).await {
        cached
    } else {
        let fresh = client
            .get_inventory_summaries()
            .await
            .map_err(|e| format!("Failed to fetch FBA inventory: {e}"))?;
        state.fba_cache().insert(cache_key, fresh.clone()).await;
        fresh
    };

    let repo = AmazonProductMappingRepository::new(state.pool());
    let mappings = repo.list().await.unwrap_or_default();

    let total_fba_units: i32 = summaries
        .iter()
        .map(|s| s.total_quantity.unwrap_or(0))
        .sum();

    // Fetch Shopify and ShipHero stock in parallel
    let shopify_stock = fetch_shopify_stock(state).await;
    let shiphero_stock = fetch_shiphero_stock(state).await;

    let mut items = Vec::new();
    let mut fba_only = Vec::new();

    for summary in &summaries {
        let sku = summary.seller_sku.as_deref().unwrap_or("");
        let mapping = mappings.iter().find(|m| m.amazon_sku == sku);

        if let Some(mapping) = mapping {
            let row = build_unified_row(summary, Some(mapping), &shopify_stock, &shiphero_stock);
            items.push(row);
        } else if !sku.is_empty() {
            fba_only.push(summary.clone());
        }
    }

    let mapped_count = items.len();

    // Also add mapped items that have no FBA inventory (merchant-fulfilled)
    add_merchant_fulfilled(
        &mappings,
        &summaries,
        &shopify_stock,
        &shiphero_stock,
        &mut items,
    );

    Ok(InventoryView {
        items,
        fba_only,
        total_fba_units,
        mapped_count,
    })
}

/// Add mapped items that have no FBA inventory (merchant-fulfilled only).
fn add_merchant_fulfilled(
    mappings: &[crate::db::AmazonProductMapping],
    summaries: &[InventorySummary],
    shopify_stock: &HashMap<String, i64>,
    shiphero_stock: &HashMap<String, i64>,
    items: &mut Vec<UnifiedInventoryRow>,
) {
    for mapping in mappings {
        let has_fba = summaries.iter().any(|s| {
            s.seller_sku
                .as_deref()
                .is_some_and(|sku| sku == mapping.amazon_sku)
        });
        if !has_fba {
            items.push(UnifiedInventoryRow {
                product_name: format!("Mapped: {}", mapping.asin),
                amazon_sku: mapping.amazon_sku.clone(),
                asin: mapping.asin.clone(),
                shopify_stock: shopify_stock.get(&mapping.shopify_product_id).copied(),
                shiphero_stock: lookup_shiphero(&mapping.amazon_sku, shiphero_stock),
                shopify_product_id: Some(mapping.shopify_product_id.clone()),
                fba_fulfillable: 0,
                fba_inbound: 0,
                fba_reserved: 0,
                fba_unfulfillable: 0,
                fba_total: 0,
            });
        }
    }
}

/// Build a unified row from an FBA summary + optional mapping.
fn build_unified_row(
    summary: &InventorySummary,
    mapping: Option<&crate::db::AmazonProductMapping>,
    shopify_stock: &HashMap<String, i64>,
    shiphero_stock: &HashMap<String, i64>,
) -> UnifiedInventoryRow {
    let details = summary.inventory_details.as_ref();

    let fba_fulfillable = details.and_then(|d| d.fulfillable_quantity).unwrap_or(0);
    let fba_inbound = details.map_or(0, |d| {
        d.inbound_receiving_quantity.unwrap_or(0)
            + d.inbound_working_quantity.unwrap_or(0)
            + d.inbound_shipped_quantity.unwrap_or(0)
    });
    let fba_reserved = details
        .and_then(|d| d.reserved_quantity.as_ref())
        .and_then(|r| r.total_reserved_quantity)
        .unwrap_or(0);
    let fba_unfulfillable = details
        .and_then(|d| d.unfulfillable_quantity.as_ref())
        .and_then(|u| u.total_unfulfillable_quantity)
        .unwrap_or(0);

    let sku = summary.seller_sku.as_deref().unwrap_or("");

    UnifiedInventoryRow {
        product_name: summary
            .product_name
            .clone()
            .unwrap_or_else(|| "Unknown".to_string()),
        amazon_sku: sku.to_string(),
        asin: summary.asin.clone().unwrap_or_default(),
        shopify_product_id: mapping.map(|m| m.shopify_product_id.clone()),
        shopify_stock: mapping.and_then(|m| shopify_stock.get(&m.shopify_product_id).copied()),
        shiphero_stock: lookup_shiphero(sku, shiphero_stock),
        fba_fulfillable,
        fba_inbound,
        fba_reserved,
        fba_unfulfillable,
        fba_total: summary.total_quantity.unwrap_or(0),
    }
}

// =============================================================================
// Cross-Channel Stock Fetchers
// =============================================================================

/// Fetch Shopify inventory indexed by product ID.
async fn fetch_shopify_stock(state: &AppState) -> HashMap<String, i64> {
    let mut stock = HashMap::new();
    let mut cursor: Option<String> = None;

    loop {
        match state.shopify().get_products(50, cursor, None).await {
            Ok(page) => {
                for product in &page.products {
                    stock.insert(product.id.clone(), product.total_inventory);
                }
                if page.page_info.has_next_page {
                    cursor = page.page_info.end_cursor;
                } else {
                    break;
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to fetch Shopify products for inventory view");
                break;
            }
        }
    }

    stock
}

/// Fetch `ShipHero` on-hand inventory indexed by SKU.
async fn fetch_shiphero_stock(state: &AppState) -> HashMap<String, i64> {
    let Some(shiphero) = state.shiphero() else {
        return HashMap::new();
    };

    let mut stock = HashMap::new();
    let mut cursor: Option<String> = None;

    loop {
        match shiphero.get_products(Some(100), cursor, None).await {
            Ok(page) => {
                for product in &page.products {
                    if let Some(sku) = &product.sku {
                        stock.insert(sku.clone(), product.total_on_hand());
                    }
                }
                if page.has_next_page {
                    cursor = page.end_cursor;
                } else {
                    break;
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to fetch ShipHero products for inventory view");
                break;
            }
        }
    }

    stock
}

/// Look up `ShipHero` stock by Amazon SKU.
fn lookup_shiphero(sku: &str, shiphero_stock: &HashMap<String, i64>) -> Option<i64> {
    if sku.is_empty() {
        return None;
    }
    shiphero_stock.get(sku).copied()
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

//! Inventory management route handlers.

use askama::Template;
use axum::{
    Form,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use tracing::instrument;

use crate::{
    filters,
    middleware::auth::RequireAdminAuth,
    shopify::types::{AdminProduct, Location, ProductStatus},
    state::AppState,
};

use naked_pineapple_core::AdminRole;

use super::dashboard::AdminUserView;

/// Low stock threshold (items below this are highlighted).
const LOW_STOCK_THRESHOLD: i64 = 10;

/// Query parameters for inventory page.
#[derive(Debug, Deserialize)]
pub struct InventoryQuery {
    pub location_id: Option<String>,
    pub cursor: Option<String>,
    pub query: Option<String>,
    pub low_stock_only: Option<bool>,
}

/// Form input for inventory adjustment.
#[derive(Debug, Deserialize)]
pub struct InventoryAdjustForm {
    pub inventory_item_id: String,
    pub location_id: String,
    pub delta: i64,
    pub reason: Option<String>,
}

/// Form input for inventory set.
#[derive(Debug, Deserialize)]
pub struct InventorySetForm {
    pub inventory_item_id: String,
    pub location_id: String,
    pub quantity: i64,
    pub reason: Option<String>,
}

/// Location view for templates.
#[derive(Debug, Clone)]
pub struct LocationView {
    pub id: String,
    pub name: String,
    pub is_active: bool,
}

impl From<&Location> for LocationView {
    fn from(loc: &Location) -> Self {
        Self {
            id: loc.id.clone(),
            name: loc.name.clone(),
            is_active: loc.is_active,
        }
    }
}

/// Inventory item view for templates.
#[derive(Debug, Clone)]
pub struct InventoryItemView {
    pub product_id: String,
    pub product_title: String,
    pub product_handle: String,
    pub product_image_url: Option<String>,
    pub variant_id: String,
    pub variant_title: String,
    pub sku: Option<String>,
    pub inventory_item_id: String,
    pub quantity: i64,
    pub is_low_stock: bool,
    pub status: String,
    pub status_class: String,
}

impl InventoryItemView {
    fn from_product(product: &AdminProduct) -> Vec<Self> {
        let (status, status_class) = match product.status {
            ProductStatus::Active => (
                "Active",
                "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
            ),
            ProductStatus::Draft => (
                "Draft",
                "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400",
            ),
            ProductStatus::Archived => (
                "Archived",
                "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-400",
            ),
            ProductStatus::Unlisted => (
                "Unlisted",
                "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
            ),
        };

        product
            .variants
            .iter()
            .map(|v| Self {
                product_id: product.id.clone(),
                product_title: product.title.clone(),
                product_handle: product.handle.clone(),
                product_image_url: product.featured_image.as_ref().map(|img| img.url.clone()),
                variant_id: v.id.clone(),
                variant_title: v.title.clone(),
                sku: v.sku.clone(),
                inventory_item_id: v.inventory_item_id.clone(),
                quantity: v.inventory_quantity,
                is_low_stock: v.inventory_quantity <= LOW_STOCK_THRESHOLD,
                status: status.to_string(),
                status_class: status_class.to_string(),
            })
            .collect()
    }
}

/// Inventory index page template.
#[derive(Template)]
#[template(path = "inventory/index.html")]
pub struct InventoryIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub locations: Vec<LocationView>,
    pub selected_location_id: Option<String>,
    pub items: Vec<InventoryItemView>,
    pub has_next_page: bool,
    pub next_cursor: Option<String>,
    pub search_query: Option<String>,
    pub low_stock_only: bool,
    pub low_stock_count: usize,
}

/// Inventory row partial template (for HTMX updates).
#[derive(Template)]
#[template(path = "inventory/_row.html")]
pub struct InventoryRowTemplate {
    pub item: InventoryItemView,
    pub selected_location_id: String,
}

/// Inventory index page handler.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<InventoryQuery>,
) -> Html<String> {
    // Fetch locations and products in parallel
    let locations_future = state.shopify().get_locations();
    let products_future =
        state
            .shopify()
            .get_products(50, query.cursor.clone(), query.query.clone());

    let (locations_result, products_result) = tokio::join!(locations_future, products_future);

    // Process locations
    let locations: Vec<LocationView> = match locations_result {
        Ok(conn) => conn
            .locations
            .iter()
            .filter(|l| l.is_active)
            .map(LocationView::from)
            .collect(),
        Err(e) => {
            tracing::error!("Failed to fetch locations: {e}");
            vec![]
        }
    };

    // Use first location as default if none selected
    let selected_location_id = query
        .location_id
        .or_else(|| locations.first().map(|l| l.id.clone()));

    // Process products into inventory items
    let (items, has_next_page, next_cursor) = match products_result {
        Ok(conn) => {
            let mut all_items: Vec<InventoryItemView> = conn
                .products
                .iter()
                .flat_map(InventoryItemView::from_product)
                .collect();

            // Filter to low stock only if requested
            if query.low_stock_only.unwrap_or(false) {
                all_items.retain(|item| item.is_low_stock);
            }

            (
                all_items,
                conn.page_info.has_next_page,
                conn.page_info.end_cursor,
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch products: {e}");
            (vec![], false, None)
        }
    };

    // Count low stock items
    let low_stock_count = items.iter().filter(|i| i.is_low_stock).count();

    let template = InventoryIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/inventory".to_string(),
        locations,
        selected_location_id,
        items,
        has_next_page,
        next_cursor,
        search_query: query.query,
        low_stock_only: query.low_stock_only.unwrap_or(false),
        low_stock_count,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Adjust inventory quantity (HTMX handler).
#[instrument(skip(_admin, state))]
pub async fn adjust(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(form): Form<InventoryAdjustForm>,
) -> impl IntoResponse {
    let reason = form
        .reason
        .as_deref()
        .unwrap_or("Manual adjustment from admin");

    match state
        .shopify()
        .adjust_inventory(
            &form.inventory_item_id,
            &form.location_id,
            form.delta,
            Some(reason),
        )
        .await
    {
        Ok(()) => {
            tracing::info!(
                inventory_item_id = %form.inventory_item_id,
                location_id = %form.location_id,
                delta = %form.delta,
                "Inventory adjusted"
            );
            // Return success indicator (HTMX will handle the update)
            (
                StatusCode::OK,
                [("HX-Trigger", "inventory-updated")],
                Html(format!(
                    r#"<span class="text-green-600 dark:text-green-400">Updated ({}{})</span>"#,
                    if form.delta >= 0 { "+" } else { "" },
                    form.delta
                )),
            )
        }
        Err(e) => {
            tracing::error!(
                inventory_item_id = %form.inventory_item_id,
                location_id = %form.location_id,
                delta = %form.delta,
                error = %e,
                "Failed to adjust inventory"
            );
            (
                StatusCode::BAD_REQUEST,
                [("HX-Trigger", "inventory-error")],
                Html(format!(
                    r#"<span class="text-red-600 dark:text-red-400">Error: {e}</span>"#
                )),
            )
        }
    }
}

/// Set inventory quantity (HTMX handler).
#[instrument(skip(_admin, state))]
pub async fn set(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(form): Form<InventorySetForm>,
) -> impl IntoResponse {
    let reason = form.reason.as_deref().unwrap_or("Manual set from admin");

    match state
        .shopify()
        .set_inventory(
            &form.inventory_item_id,
            &form.location_id,
            form.quantity,
            Some(reason),
        )
        .await
    {
        Ok(()) => {
            tracing::info!(
                inventory_item_id = %form.inventory_item_id,
                location_id = %form.location_id,
                quantity = %form.quantity,
                "Inventory set"
            );
            (
                StatusCode::OK,
                [("HX-Trigger", "inventory-updated")],
                Html(format!(
                    r#"<span class="text-green-600 dark:text-green-400">Set to {}</span>"#,
                    form.quantity
                )),
            )
        }
        Err(e) => {
            tracing::error!(
                inventory_item_id = %form.inventory_item_id,
                location_id = %form.location_id,
                quantity = %form.quantity,
                error = %e,
                "Failed to set inventory"
            );
            (
                StatusCode::BAD_REQUEST,
                [("HX-Trigger", "inventory-error")],
                Html(format!(
                    r#"<span class="text-red-600 dark:text-red-400">Error: {e}</span>"#
                )),
            )
        }
    }
}

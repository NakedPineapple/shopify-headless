//! Inventory management route handlers.

use askama::Template;
use axum::{
    Form,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use std::collections::HashMap;
use tracing::instrument;

use crate::{
    components::data_table::{DataTableConfig, FilterType, inventory_table_config},
    filters,
    middleware::auth::RequireAdminAuth,
    shopify::types::{
        InventoryItem, InventoryItemConnection, InventoryItemUpdateInput, Location, ProductStatus,
    },
    state::AppState,
};

use naked_pineapple_core::AdminRole;

use super::dashboard::AdminUserView;

/// Low stock threshold (items below this are highlighted).
const LOW_STOCK_THRESHOLD: i64 = 10;

// =============================================================================
// Query Parameters
// =============================================================================

/// Query parameters for inventory page.
#[derive(Debug, Deserialize)]
pub struct InventoryQuery {
    pub location_id: Option<String>,
    pub cursor: Option<String>,
    pub query: Option<String>,
    pub tracking: Option<String>,
    pub stock_status: Option<String>,
    pub product_status: Option<String>,
    pub sort: Option<String>,
    pub dir: Option<String>,
}

// =============================================================================
// Form Inputs
// =============================================================================

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

/// Form input for inventory move between locations.
#[derive(Debug, Deserialize)]
pub struct InventoryMoveForm {
    pub inventory_item_id: String,
    pub from_location_id: String,
    pub to_location_id: String,
    pub quantity: i64,
    pub reason: Option<String>,
}

/// Form input for activating inventory at a location.
#[derive(Debug, Deserialize)]
pub struct InventoryActivateForm {
    pub inventory_item_id: String,
    pub location_id: String,
}

/// Form input for deactivating inventory at a location.
#[derive(Debug, Deserialize)]
pub struct InventoryDeactivateForm {
    pub inventory_level_id: String,
    pub location_id: String,
}

// =============================================================================
// View Types
// =============================================================================

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
    pub id: String,
    /// Alias for id for template compatibility.
    pub inventory_item_id: String,
    pub product_id: Option<String>,
    pub product_title: String,
    pub product_handle: Option<String>,
    pub product_image_url: Option<String>,
    pub variant_id: Option<String>,
    /// Variant title (defaults to "Default Title" for single variants).
    pub variant_title: String,
    pub sku: Option<String>,
    pub tracked: bool,
    pub on_hand: i64,
    pub available: i64,
    /// Alias for available for template compatibility.
    pub quantity: i64,
    pub committed: i64,
    pub incoming: i64,
    pub unit_cost: Option<String>,
    pub is_low_stock: bool,
    pub is_out_of_stock: bool,
    pub status: String,
    pub status_class: String,
}

/// Extract product information from an inventory item.
fn extract_product_info(
    item: &InventoryItem,
) -> (
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
) {
    let Some(ref v) = item.variant else {
        return (
            None,
            "Unknown Product".to_string(),
            None,
            None,
            "-".to_string(),
            String::new(),
        );
    };

    let Some(ref p) = v.product else {
        return (
            None,
            "Unknown Product".to_string(),
            None,
            None,
            "-".to_string(),
            String::new(),
        );
    };

    let (status_text, status_css) = match p.status {
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

    (
        Some(p.id.clone()),
        p.title.clone(),
        Some(p.handle.clone()),
        p.featured_image.as_ref().map(|img| img.url.clone()),
        status_text.to_string(),
        status_css.to_string(),
    )
}

/// Ensure ID has the proper Shopify GID format for inventory items.
fn normalize_inventory_item_id(id: &str) -> String {
    if id.starts_with("gid://") {
        id.to_string()
    } else {
        format!("gid://shopify/InventoryItem/{id}")
    }
}

impl From<&InventoryItem> for InventoryItemView {
    fn from(item: &InventoryItem) -> Self {
        // Sum quantities across all locations
        let on_hand: i64 = item.inventory_levels.iter().map(|l| l.on_hand).sum();
        let available: i64 = item.inventory_levels.iter().map(|l| l.available).sum();
        let incoming: i64 = item.inventory_levels.iter().map(|l| l.incoming).sum();
        // Committed is on_hand - available
        let committed = on_hand.saturating_sub(available);

        // Extract product/variant info
        let (product_id, product_title, product_handle, product_image_url, status, status_class) =
            extract_product_info(item);

        let variant_id = item.variant.as_ref().map(|v| v.id.clone());
        let variant_title = item
            .variant
            .as_ref()
            .map_or_else(|| "Default Title".to_string(), |v| v.title.clone());

        Self {
            id: item.id.clone(),
            inventory_item_id: item.id.clone(),
            product_id,
            product_title,
            product_handle,
            product_image_url,
            variant_id,
            variant_title,
            sku: item.sku.clone(),
            tracked: item.tracked,
            on_hand,
            available,
            quantity: available,
            committed,
            incoming,
            unit_cost: item
                .unit_cost
                .as_ref()
                .map(|c| format!("{} {}", c.amount, c.currency_code)),
            is_low_stock: available > 0 && available <= LOW_STOCK_THRESHOLD,
            is_out_of_stock: available == 0,
            status,
            status_class,
        }
    }
}

/// Column visibility helper for templates.
///
/// This struct intentionally uses multiple boolean fields to track visibility
/// of each table column. A more complex state machine would be overkill for this
/// simple visibility toggle use case.
#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct ColumnVisibility {
    pub product: bool,
    pub sku: bool,
    pub tracked: bool,
    pub on_hand: bool,
    pub available: bool,
    pub committed: bool,
    pub incoming: bool,
    pub cost: bool,
    pub status: bool,
}

impl ColumnVisibility {
    fn new(visible: &[&str]) -> Self {
        Self {
            product: visible.contains(&"product"),
            sku: visible.contains(&"sku"),
            tracked: visible.contains(&"tracked"),
            on_hand: visible.contains(&"on_hand"),
            available: visible.contains(&"available"),
            committed: visible.contains(&"committed"),
            incoming: visible.contains(&"incoming"),
            cost: visible.contains(&"cost"),
            status: visible.contains(&"status"),
        }
    }

    /// Check if a column is visible by key.
    #[must_use]
    pub fn is_visible(&self, key: &str) -> bool {
        match key {
            "product" => self.product,
            "sku" => self.sku,
            "tracked" => self.tracked,
            "on_hand" => self.on_hand,
            "available" => self.available,
            "committed" => self.committed,
            "incoming" => self.incoming,
            "cost" => self.cost,
            "status" => self.status,
            _ => false,
        }
    }
}

// =============================================================================
// Templates
// =============================================================================

/// Inventory index page template.
#[derive(Template)]
#[template(path = "inventory/index.html")]
pub struct InventoryIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    // Locations
    pub locations: Vec<LocationView>,
    pub selected_location_id: Option<String>,
    // Data table data
    pub items: Vec<InventoryItemView>,
    pub has_next_page: bool,
    pub next_cursor: Option<String>,
    // Data table configuration
    pub table_config: DataTableConfig,
    pub col_visible: ColumnVisibility,
    // Current filter/sort state (using template-compatible names)
    pub search_query: Option<String>,
    pub filter_values: HashMap<String, String>,
    pub sort_column: Option<String>,
    pub sort_direction: String,
    // Stats
    pub low_stock_count: usize,
    pub out_of_stock_count: usize,
    // Legacy filter for existing template
    pub low_stock_only: bool,
    // Preserve URL params for links
    pub preserve_params: String,
}

/// Inventory row partial template (for HTMX updates).
#[derive(Template)]
#[template(path = "inventory/_row.html")]
pub struct InventoryRowTemplate {
    pub item: InventoryItemView,
    /// Selected location ID (for form submissions).
    pub selected_location_id: Option<String>,
    pub col_visible: ColumnVisibility,
}

/// Detailed inventory item view for show/edit pages.
#[derive(Debug, Clone)]
pub struct InventoryItemDetailView {
    pub id: String,
    pub product_id: Option<String>,
    pub product_title: String,
    pub product_handle: Option<String>,
    pub product_image_url: Option<String>,
    pub variant_id: Option<String>,
    pub variant_title: String,
    pub sku: Option<String>,
    pub tracked: bool,
    pub requires_shipping: bool,
    pub unit_cost: Option<String>,
    pub unit_cost_amount: Option<String>,
    pub unit_cost_currency: Option<String>,
    pub harmonized_system_code: Option<String>,
    pub country_code_of_origin: Option<String>,
    pub province_code_of_origin: Option<String>,
    pub status: String,
    pub status_class: String,
    pub inventory_levels: Vec<InventoryLevelView>,
    pub total_on_hand: i64,
    pub total_available: i64,
}

/// Inventory level view for detail pages.
#[derive(Debug, Clone)]
pub struct InventoryLevelView {
    pub location_id: String,
    pub location_name: String,
    pub available: i64,
    pub on_hand: i64,
    pub committed: i64,
    pub incoming: i64,
    pub reserved: i64,
    pub damaged: i64,
    pub updated_at: Option<String>,
}

impl From<&InventoryItem> for InventoryItemDetailView {
    fn from(item: &InventoryItem) -> Self {
        let (product_id, product_title, product_handle, product_image_url, status, status_class) =
            extract_product_info(item);

        let variant_id = item.variant.as_ref().map(|v| v.id.clone());
        let variant_title = item
            .variant
            .as_ref()
            .map_or_else(|| "Default Title".to_string(), |v| v.title.clone());

        // Parse unit cost
        let (unit_cost, unit_cost_amount, unit_cost_currency) =
            item.unit_cost.as_ref().map_or((None, None, None), |cost| {
                (
                    Some(format!("{} {}", cost.amount, cost.currency_code)),
                    Some(cost.amount.clone()),
                    Some(cost.currency_code.clone()),
                )
            });

        // Convert inventory levels
        let inventory_levels: Vec<InventoryLevelView> = item
            .inventory_levels
            .iter()
            .map(|level| {
                let available = level.available;
                let on_hand = level.on_hand;
                let incoming = level.incoming;
                let committed = on_hand.saturating_sub(available);

                InventoryLevelView {
                    location_id: level.location_id.clone(),
                    location_name: level.location_name.clone().unwrap_or_default(),
                    available,
                    on_hand,
                    committed,
                    incoming,
                    reserved: 0,
                    damaged: 0,
                    updated_at: level.updated_at.clone(),
                }
            })
            .collect();

        let total_on_hand: i64 = inventory_levels.iter().map(|l| l.on_hand).sum();
        let total_available: i64 = inventory_levels.iter().map(|l| l.available).sum();

        Self {
            id: item.id.clone(),
            product_id,
            product_title,
            product_handle,
            product_image_url,
            variant_id,
            variant_title,
            sku: item.sku.clone(),
            tracked: item.tracked,
            requires_shipping: item.requires_shipping,
            unit_cost,
            unit_cost_amount,
            unit_cost_currency,
            harmonized_system_code: item.harmonized_system_code.clone(),
            country_code_of_origin: item.country_code_of_origin.clone(),
            province_code_of_origin: item.province_code_of_origin.clone(),
            status,
            status_class,
            inventory_levels,
            total_on_hand,
            total_available,
        }
    }
}

/// Inventory item detail page template.
#[derive(Template)]
#[template(path = "inventory/show.html")]
pub struct InventoryShowTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub item: InventoryItemDetailView,
    pub locations: Vec<LocationView>,
}

/// Inventory item edit page template.
#[derive(Template)]
#[template(path = "inventory/edit.html")]
pub struct InventoryEditTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub item: InventoryItemDetailView,
}

/// Form input for inventory item update.
#[derive(Debug, Deserialize)]
pub struct InventoryUpdateForm {
    pub tracked: Option<String>,
    pub requires_shipping: Option<String>,
    pub harmonized_system_code: Option<String>,
    pub country_code_of_origin: Option<String>,
    pub province_code_of_origin: Option<String>,
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Build Shopify query string from filter parameters.
fn build_shopify_query(query: &InventoryQuery) -> Option<String> {
    let mut parts = Vec::new();

    // Text search
    if let Some(ref q) = query.query
        && !q.is_empty()
    {
        parts.push(q.clone());
    }

    // Tracking filter
    if let Some(ref tracking) = query.tracking {
        match tracking.as_str() {
            "tracked" => parts.push("tracked:true".to_string()),
            "untracked" => parts.push("tracked:false".to_string()),
            _ => {}
        }
    }

    // Product status filter
    if let Some(ref status) = query.product_status {
        let statuses: Vec<&str> = status.split(',').collect();
        if !statuses.is_empty() {
            let status_query = statuses
                .iter()
                .map(|s| format!("product_status:{s}"))
                .collect::<Vec<_>>()
                .join(" OR ");
            parts.push(format!("({status_query})"));
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" AND "))
    }
}

/// Apply stock status filter to items.
fn apply_stock_status_filter(items: &mut Vec<InventoryItemView>, stock_status: Option<&str>) {
    let Some(stock_status) = stock_status else {
        return;
    };

    let statuses: Vec<&str> = stock_status.split(',').collect();
    items.retain(|item| {
        statuses.iter().any(|status| match *status {
            "in_stock" => !item.is_out_of_stock && !item.is_low_stock,
            "low_stock" => item.is_low_stock,
            "out_of_stock" => item.is_out_of_stock,
            _ => false,
        })
    });
}

/// Build filter values map from query parameters.
fn build_filter_values(query: &InventoryQuery) -> HashMap<String, String> {
    let mut filter_values = HashMap::new();
    if let Some(ref v) = query.tracking {
        filter_values.insert("tracking".to_string(), v.clone());
    }
    if let Some(ref v) = query.stock_status {
        filter_values.insert("stock_status".to_string(), v.clone());
    }
    if let Some(ref v) = query.product_status {
        filter_values.insert("product_status".to_string(), v.clone());
    }
    filter_values
}

/// Build URL params string for preserving state.
fn build_preserve_params(query: &InventoryQuery) -> String {
    let mut params = Vec::new();

    if let Some(ref v) = query.location_id {
        params.push(format!("location_id={v}"));
    }
    if let Some(ref v) = query.query {
        params.push(format!("query={}", urlencoding::encode(v)));
    }
    if let Some(ref v) = query.tracking {
        params.push(format!("tracking={v}"));
    }
    if let Some(ref v) = query.stock_status {
        params.push(format!("stock_status={v}"));
    }
    if let Some(ref v) = query.product_status {
        params.push(format!("product_status={v}"));
    }
    if let Some(ref v) = query.sort {
        params.push(format!("sort={v}"));
    }
    if let Some(ref v) = query.dir {
        params.push(format!("dir={v}"));
    }

    if params.is_empty() {
        String::new()
    } else {
        format!("&{}", params.join("&"))
    }
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /inventory - Inventory list page.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<InventoryQuery>,
) -> Html<String> {
    // Fetch locations and inventory items in parallel
    let locations_future = state.shopify().get_locations();
    let shopify_query = build_shopify_query(&query);
    let items_future = state
        .shopify()
        .get_inventory_items(50, query.cursor.clone(), shopify_query);

    let (locations_result, items_result) = tokio::join!(locations_future, items_future);

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
        .clone()
        .or_else(|| locations.first().map(|l| l.id.clone()));

    // Process inventory items
    let (mut items, has_next_page, next_cursor) = match items_result {
        Ok(conn) => {
            let items: Vec<InventoryItemView> =
                conn.items.iter().map(InventoryItemView::from).collect();
            (
                items,
                conn.page_info.has_next_page,
                conn.page_info.end_cursor,
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch inventory items: {e}");
            (vec![], false, None)
        }
    };

    // Apply client-side stock status filter (API doesn't support this directly)
    apply_stock_status_filter(&mut items, query.stock_status.as_deref());

    // Calculate stats
    let low_stock_count = items.iter().filter(|i| i.is_low_stock).count();
    let out_of_stock_count = items.iter().filter(|i| i.is_out_of_stock).count();

    // Get table configuration and visible columns
    let table_config = inventory_table_config();
    let visible_columns: Vec<&str> = table_config
        .columns
        .iter()
        .filter(|c| c.default_visible)
        .map(|c| c.key.as_str())
        .collect();
    let col_visible = ColumnVisibility::new(&visible_columns);

    // Build filter values and determine if low_stock_only filter is active
    let filter_values = build_filter_values(&query);
    let low_stock_only = query.stock_status.as_deref() == Some("low_stock");

    let preserve_params = build_preserve_params(&query);

    let template = InventoryIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/inventory".to_string(),
        locations,
        selected_location_id,
        items,
        has_next_page,
        next_cursor,
        table_config,
        col_visible,
        search_query: query.query,
        filter_values,
        sort_column: query.sort,
        sort_direction: query.dir.unwrap_or_else(|| "asc".to_string()),
        low_stock_count,
        out_of_stock_count,
        low_stock_only,
        preserve_params,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// POST /inventory/adjust - Adjust inventory quantity (HTMX handler).
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

/// POST /inventory/set - Set inventory quantity (HTMX handler).
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

/// GET /inventory/:id - Inventory item detail page.
#[instrument(skip(admin, state))]
pub async fn show(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let inventory_item_id = normalize_inventory_item_id(&id);

    // Fetch inventory item and locations in parallel
    let item_future = state.shopify().get_inventory_item(&inventory_item_id);
    let locations_future = state.shopify().get_locations();

    let (item_result, locations_result) = tokio::join!(item_future, locations_future);

    let item = match item_result {
        Ok(item) => InventoryItemDetailView::from(&item),
        Err(e) => {
            tracing::error!(id = %id, error = %e, "Failed to fetch inventory item");
            return (
                StatusCode::NOT_FOUND,
                Html("Inventory item not found".to_string()),
            )
                .into_response();
        }
    };

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

    let template = InventoryShowTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: format!("/inventory/{id}"),
        item,
        locations,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

/// GET /inventory/:id/edit - Inventory item edit page.
#[instrument(skip(admin, state))]
pub async fn edit(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let inventory_item_id = normalize_inventory_item_id(&id);

    let item = match state.shopify().get_inventory_item(&inventory_item_id).await {
        Ok(item) => InventoryItemDetailView::from(&item),
        Err(e) => {
            tracing::error!(id = %id, error = %e, "Failed to fetch inventory item");
            return (
                StatusCode::NOT_FOUND,
                Html("Inventory item not found".to_string()),
            )
                .into_response();
        }
    };

    let template = InventoryEditTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: format!("/inventory/{id}/edit"),
        item,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

/// POST /inventory/:id - Update inventory item.
#[instrument(skip(_admin, state))]
pub async fn update(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<InventoryUpdateForm>,
) -> impl IntoResponse {
    let inventory_item_id = normalize_inventory_item_id(&id);

    let input = InventoryItemUpdateInput {
        sku: None, // SKU updates require variant mutation
        tracked: form.tracked.as_deref().map(|v| v == "on" || v == "true"),
        requires_shipping: form
            .requires_shipping
            .as_deref()
            .map(|v| v == "on" || v == "true"),
        cost: None, // Cost updates handled separately
        harmonized_system_code: form.harmonized_system_code.clone(),
        country_code_of_origin: form.country_code_of_origin.clone(),
        province_code_of_origin: form.province_code_of_origin.clone(),
    };

    match state
        .shopify()
        .update_inventory_item(&inventory_item_id, &input)
        .await
    {
        Ok(_) => {
            tracing::info!(id = %id, "Inventory item updated");
            axum::response::Redirect::to(&format!("/inventory/{id}")).into_response()
        }
        Err(e) => {
            tracing::error!(id = %id, error = %e, "Failed to update inventory item");
            (
                StatusCode::BAD_REQUEST,
                Html(format!("Failed to update: {e}")),
            )
                .into_response()
        }
    }
}

/// POST /inventory/move - Move inventory between locations (HTMX handler).
#[instrument(skip(_admin, state))]
pub async fn move_quantity(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(form): Form<InventoryMoveForm>,
) -> impl IntoResponse {
    let reason = form
        .reason
        .as_deref()
        .unwrap_or("Stock transfer from admin");

    match state
        .shopify()
        .move_inventory(
            &form.inventory_item_id,
            &form.from_location_id,
            &form.to_location_id,
            form.quantity,
            Some(reason),
        )
        .await
    {
        Ok(()) => {
            tracing::info!(
                inventory_item_id = %form.inventory_item_id,
                from_location = %form.from_location_id,
                to_location = %form.to_location_id,
                quantity = %form.quantity,
                "Inventory moved"
            );
            (
                StatusCode::OK,
                [("HX-Trigger", "inventory-updated")],
                Html(format!(
                    r#"<span class="text-green-600 dark:text-green-400">Moved {} units</span>"#,
                    form.quantity
                )),
            )
        }
        Err(e) => {
            tracing::error!(
                inventory_item_id = %form.inventory_item_id,
                from_location = %form.from_location_id,
                to_location = %form.to_location_id,
                quantity = %form.quantity,
                error = %e,
                "Failed to move inventory"
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

/// POST /inventory/:id/activate - Activate inventory at a location.
#[instrument(skip(_admin, state))]
pub async fn activate(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<InventoryActivateForm>,
) -> impl IntoResponse {
    match state
        .shopify()
        .activate_inventory(&form.inventory_item_id, &form.location_id)
        .await
    {
        Ok(()) => {
            tracing::info!(
                inventory_item_id = %form.inventory_item_id,
                location_id = %form.location_id,
                "Inventory activated at location"
            );
            axum::response::Redirect::to(&format!("/inventory/{id}")).into_response()
        }
        Err(e) => {
            tracing::error!(
                inventory_item_id = %form.inventory_item_id,
                location_id = %form.location_id,
                error = %e,
                "Failed to activate inventory"
            );
            (
                StatusCode::BAD_REQUEST,
                Html(format!("Failed to activate: {e}")),
            )
                .into_response()
        }
    }
}

/// POST /inventory/:id/deactivate - Deactivate inventory at a location.
#[instrument(skip(_admin, state))]
pub async fn deactivate(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<InventoryDeactivateForm>,
) -> impl IntoResponse {
    match state
        .shopify()
        .deactivate_inventory(&form.inventory_level_id)
        .await
    {
        Ok(()) => {
            tracing::info!(
                inventory_level_id = %form.inventory_level_id,
                location_id = %form.location_id,
                "Inventory deactivated at location"
            );
            axum::response::Redirect::to(&format!("/inventory/{id}")).into_response()
        }
        Err(e) => {
            tracing::error!(
                inventory_level_id = %form.inventory_level_id,
                location_id = %form.location_id,
                error = %e,
                "Failed to deactivate inventory"
            );
            (
                StatusCode::BAD_REQUEST,
                Html(format!("Failed to deactivate: {e}")),
            )
                .into_response()
        }
    }
}

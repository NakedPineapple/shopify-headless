//! Warehouse route handlers.
//!
//! Provides visibility into the `ShipHero` warehouse management system,
//! including orders awaiting fulfillment and shipment history.

use askama::Template;
use axum::{
    Router,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use serde::Deserialize;
use tower_sessions::Session;
use tracing::instrument;

use crate::filters;
use crate::models::CurrentAdmin;
use crate::shiphero::{
    OrderConnection, OrderHistoryEvent, Product, Shipment, ShipmentConnection, WarehouseOrder,
    WarehouseOrderDetail,
};
use crate::state::AppState;

use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Warehouse orders list template.
#[derive(Template)]
#[template(path = "warehouse/orders/index.html")]
pub struct WarehouseOrdersTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub orders: Vec<WarehouseOrder>,
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
    pub fulfillment_status: String,
    pub error_message: Option<String>,
}

/// Warehouse order detail template.
#[derive(Template)]
#[template(path = "warehouse/orders/show.html")]
pub struct WarehouseOrderDetailTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub order: WarehouseOrderDetail,
    pub history: Vec<OrderHistoryEvent>,
    pub error_message: Option<String>,
}

/// Warehouse shipments list template.
#[derive(Template)]
#[template(path = "warehouse/shipments/index.html")]
pub struct WarehouseShipmentsTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub shipments: Vec<Shipment>,
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
    pub error_message: Option<String>,
}

/// Warehouse shipment detail template.
#[derive(Template)]
#[template(path = "warehouse/shipments/show.html")]
pub struct WarehouseShipmentDetailTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub shipment: Shipment,
    pub error_message: Option<String>,
}

/// Not connected template shown when `ShipHero` is not configured.
#[derive(Template)]
#[template(path = "warehouse/not_connected.html")]
pub struct NotConnectedTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
}

/// Warehouse dashboard template.
#[derive(Template)]
#[template(path = "warehouse/dashboard.html")]
pub struct WarehouseDashboardTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub pending_orders_count: usize,
    pub shipped_today_count: usize,
    pub low_stock_count: usize,
    pub total_products: usize,
    pub items_in_stock: usize,
    pub items_at_zero: usize,
    pub recent_orders: Vec<WarehouseOrder>,
    pub recent_shipments: Vec<Shipment>,
    pub low_stock_items: Vec<LowStockItem>,
    pub error_message: Option<String>,
}

/// Low stock item for dashboard display.
#[derive(Debug, Clone)]
pub struct LowStockItem {
    /// Product SKU.
    pub sku: String,
    /// Product name.
    pub name: String,
    /// Available quantity.
    pub available: i64,
    /// Reorder level.
    pub reorder_level: i64,
}

/// Warehouse inventory list template.
#[derive(Template)]
#[template(path = "warehouse/inventory/index.html")]
pub struct WarehouseInventoryTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub products: Vec<Product>,
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
    pub search_query: Option<String>,
    pub error_message: Option<String>,
}

// =============================================================================
// Query Parameters
// =============================================================================

/// Query parameters for orders list.
#[derive(Debug, Deserialize)]
pub struct OrdersQuery {
    /// Pagination cursor.
    pub cursor: Option<String>,
    /// Fulfillment status filter.
    pub status: Option<String>,
}

/// Query parameters for shipments list.
#[derive(Debug, Deserialize)]
pub struct ShipmentsQuery {
    /// Pagination cursor.
    pub cursor: Option<String>,
    /// Date filter: "today", "week", "month", or custom.
    pub range: Option<String>,
}

/// Query parameters for inventory list.
#[derive(Debug, Deserialize)]
pub struct InventoryQuery {
    /// Pagination cursor.
    pub cursor: Option<String>,
    /// Search by SKU.
    pub sku: Option<String>,
}

// =============================================================================
// Router
// =============================================================================

/// Build the warehouse router.
pub fn router() -> Router<AppState> {
    Router::new()
        // Warehouse dashboard
        .route("/warehouse", get(dashboard))
        // Warehouse inventory
        .route("/warehouse/inventory", get(inventory_index))
        // Warehouse orders
        .route("/warehouse/orders", get(orders_index))
        .route("/warehouse/orders/{id}", get(orders_show))
        // Warehouse shipments
        .route("/warehouse/shipments", get(shipments_index))
        .route("/warehouse/shipments/{id}", get(shipments_show))
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Get the current admin user from session.
async fn get_current_admin(session: &Session) -> Option<CurrentAdmin> {
    session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
}

// =============================================================================
// Dashboard Route
// =============================================================================

/// Aggregated dashboard data from `ShipHero` API calls.
struct DashboardData {
    pending_orders_count: usize,
    shipped_today_count: usize,
    low_stock_count: usize,
    total_products: usize,
    items_in_stock: usize,
    items_at_zero: usize,
    recent_orders: Vec<WarehouseOrder>,
    recent_shipments: Vec<Shipment>,
    low_stock_items: Vec<LowStockItem>,
    error_message: Option<String>,
}

/// Fetch and process all dashboard data from `ShipHero`.
async fn fetch_dashboard_data(client: &crate::shiphero::ShipHeroClient) -> DashboardData {
    let (orders_result, shipments_result, low_stock_result, health_result) = tokio::join!(
        client.get_pending_orders(Some(10), None, Some("pending".to_string())),
        client.get_shipments(Some(10), None, None, None),
        client.get_low_stock(Some(10), None),
        client.get_inventory_health(),
    );

    let mut error_message = None;

    let (pending_orders_count, recent_orders) = orders_result.map_or_else(
        |e| {
            tracing::error!(error = %e, "Failed to fetch pending orders");
            error_message = Some(format!("Failed to fetch pending orders: {e}"));
            (0, Vec::new())
        },
        |conn| (conn.orders.len(), conn.orders),
    );

    let (shipped_today_count, recent_shipments) = shipments_result.map_or_else(
        |e| {
            tracing::error!(error = %e, "Failed to fetch shipments");
            error_message.get_or_insert_with(|| format!("Failed to fetch shipments: {e}"));
            (0, Vec::new())
        },
        |conn| {
            let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
            let count = conn
                .shipments
                .iter()
                .filter(|s| {
                    s.created_date
                        .as_ref()
                        .is_some_and(|d| d.starts_with(&today))
                })
                .count();
            (count, conn.shipments)
        },
    );

    let low_stock_items: Vec<LowStockItem> = low_stock_result.map_or_else(
        |e| {
            tracing::error!(error = %e, "Failed to fetch low stock products");
            error_message.get_or_insert_with(|| format!("Failed to fetch low stock products: {e}"));
            Vec::new()
        },
        |conn| {
            conn.products
                .into_iter()
                .map(|p| LowStockItem {
                    sku: p.sku,
                    name: p.name,
                    available: p.on_hand,
                    reorder_level: p.reorder_level,
                })
                .collect()
        },
    );

    let (total_products, items_in_stock, items_at_zero, low_stock_count) = health_result
        .map_or_else(
            |e| {
                tracing::error!(error = %e, "Failed to fetch inventory health");
                error_message
                    .get_or_insert_with(|| format!("Failed to fetch inventory health: {e}"));
                (0, 0, 0, low_stock_items.len())
            },
            |h| {
                (
                    h.total_skus,
                    h.items_in_stock,
                    h.items_at_zero,
                    h.low_stock_count,
                )
            },
        );

    DashboardData {
        pending_orders_count,
        shipped_today_count,
        low_stock_count,
        total_products,
        items_in_stock,
        items_at_zero,
        recent_orders,
        recent_shipments,
        low_stock_items,
        error_message,
    }
}

/// GET /warehouse - Warehouse dashboard with key metrics.
#[instrument(skip(state, session))]
pub async fn dashboard(State(state): State<AppState>, session: Session) -> Response {
    let Some(admin) = get_current_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let Some(client) = state.shiphero() else {
        let template = NotConnectedTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/warehouse".to_string(),
        };
        return Html(template.render().unwrap_or_else(|e| {
            tracing::error!("Template render error: {}", e);
            "Internal Server Error".to_string()
        }))
        .into_response();
    };

    let data = fetch_dashboard_data(client).await;

    let template = WarehouseDashboardTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/warehouse".to_string(),
        pending_orders_count: data.pending_orders_count,
        shipped_today_count: data.shipped_today_count,
        low_stock_count: data.low_stock_count,
        total_products: data.total_products,
        items_in_stock: data.items_in_stock,
        items_at_zero: data.items_at_zero,
        recent_orders: data.recent_orders,
        recent_shipments: data.recent_shipments,
        low_stock_items: data.low_stock_items,
        error_message: data.error_message,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

// =============================================================================
// Inventory Route
// =============================================================================

/// GET /warehouse/inventory - List inventory with bin locations.
#[instrument(skip(state, session))]
pub async fn inventory_index(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<InventoryQuery>,
) -> Response {
    // Get current admin from session
    let Some(admin) = get_current_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    // Check if ShipHero is connected
    let Some(client) = state.shiphero() else {
        let template = NotConnectedTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/warehouse/inventory".to_string(),
        };
        return Html(template.render().unwrap_or_else(|e| {
            tracing::error!("Template render error: {}", e);
            "Internal Server Error".to_string()
        }))
        .into_response();
    };

    // Fetch products from ShipHero
    let result = client
        .get_products(Some(50), params.cursor.clone(), params.sku.clone())
        .await;

    let (products, has_next_page, end_cursor, error_message) = match result {
        Ok(conn) => (conn.products, conn.has_next_page, conn.end_cursor, None),
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch ShipHero products");
            (
                Vec::new(),
                false,
                None,
                Some(format!("Failed to fetch products: {e}")),
            )
        }
    };

    let template = WarehouseInventoryTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/warehouse/inventory".to_string(),
        products,
        has_next_page,
        end_cursor,
        search_query: params.sku,
        error_message,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

// =============================================================================
// Order Routes
// =============================================================================

/// GET /warehouse/orders - List orders awaiting fulfillment.
#[instrument(skip(state, session))]
pub async fn orders_index(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<OrdersQuery>,
) -> Response {
    // Get current admin from session
    let Some(admin) = get_current_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    // Check if ShipHero is connected
    let Some(client) = state.shiphero() else {
        let template = NotConnectedTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/warehouse/orders".to_string(),
        };
        return Html(template.render().unwrap_or_else(|e| {
            tracing::error!("Template render error: {}", e);
            "Internal Server Error".to_string()
        }))
        .into_response();
    };

    let fulfillment_status = params
        .status
        .clone()
        .unwrap_or_else(|| "pending".to_string());

    // Fetch orders from ShipHero
    let result = client
        .get_pending_orders(
            Some(50),
            params.cursor.clone(),
            Some(fulfillment_status.clone()),
        )
        .await;

    let (orders, has_next_page, end_cursor, error_message) = match result {
        Ok(conn) => (conn.orders, conn.has_next_page, conn.end_cursor, None),
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch ShipHero orders");
            (
                Vec::new(),
                false,
                None,
                Some(format!("Failed to fetch orders: {e}")),
            )
        }
    };

    let template = WarehouseOrdersTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/warehouse/orders".to_string(),
        orders,
        has_next_page,
        end_cursor,
        fulfillment_status,
        error_message,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

/// GET /warehouse/orders/{id} - Show order detail.
#[instrument(skip(state, session))]
pub async fn orders_show(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Response {
    // Get current admin from session
    let Some(admin) = get_current_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    // Check if ShipHero is connected
    let Some(client) = state.shiphero() else {
        let template = NotConnectedTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: format!("/warehouse/orders/{id}"),
        };
        return Html(template.render().unwrap_or_else(|e| {
            tracing::error!("Template render error: {}", e);
            "Internal Server Error".to_string()
        }))
        .into_response();
    };

    // Fetch order from ShipHero
    let order_result = client.get_order(&id).await;
    let history_result = client.get_order_history(&id).await;

    let (order, history, error_message) = match (order_result, history_result) {
        (Ok(Some(order)), Ok(history)) => (order, history, None),
        (Ok(Some(order)), Err(e)) => {
            tracing::warn!(error = %e, "Failed to fetch order history");
            (
                order,
                Vec::new(),
                Some(format!("Warning: Could not load order history: {e}")),
            )
        }
        (Ok(None), _) => {
            return Redirect::to("/warehouse/orders?error=not_found").into_response();
        }
        (Err(e), _) => {
            tracing::error!(error = %e, "Failed to fetch ShipHero order");
            return Redirect::to(&format!(
                "/warehouse/orders?error={}",
                urlencoding::encode(&e.to_string())
            ))
            .into_response();
        }
    };

    let template = WarehouseOrderDetailTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: format!("/warehouse/orders/{id}"),
        order,
        history,
        error_message,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

// =============================================================================
// Shipment Routes
// =============================================================================

/// GET /warehouse/shipments - List recent shipments.
#[instrument(skip(state, session))]
pub async fn shipments_index(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<ShipmentsQuery>,
) -> Response {
    // Get current admin from session
    let Some(admin) = get_current_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    // Check if ShipHero is connected
    let Some(client) = state.shiphero() else {
        let template = NotConnectedTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/warehouse/shipments".to_string(),
        };
        return Html(template.render().unwrap_or_else(|e| {
            tracing::error!("Template render error: {}", e);
            "Internal Server Error".to_string()
        }))
        .into_response();
    };

    // Calculate date range based on filter
    let (date_from, date_to) = match params.range.as_deref() {
        Some("today") => {
            let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
            (Some(today.clone()), Some(today))
        }
        Some("week") => {
            let now = chrono::Utc::now();
            let week_ago = now - chrono::Duration::days(7);
            (
                Some(week_ago.format("%Y-%m-%d").to_string()),
                Some(now.format("%Y-%m-%d").to_string()),
            )
        }
        Some("month") | None => {
            let now = chrono::Utc::now();
            let month_ago = now - chrono::Duration::days(30);
            (
                Some(month_ago.format("%Y-%m-%d").to_string()),
                Some(now.format("%Y-%m-%d").to_string()),
            )
        }
        _ => (None, None),
    };

    // Fetch shipments from ShipHero
    let result = client
        .get_shipments(Some(50), params.cursor.clone(), date_from, date_to)
        .await;

    let (shipments, has_next_page, end_cursor, error_message) = match result {
        Ok(conn) => (conn.shipments, conn.has_next_page, conn.end_cursor, None),
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch ShipHero shipments");
            (
                Vec::new(),
                false,
                None,
                Some(format!("Failed to fetch shipments: {e}")),
            )
        }
    };

    let template = WarehouseShipmentsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/warehouse/shipments".to_string(),
        shipments,
        has_next_page,
        end_cursor,
        error_message,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

/// GET /warehouse/shipments/{id} - Show shipment detail.
#[instrument(skip(state, session))]
pub async fn shipments_show(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Response {
    // Get current admin from session
    let Some(admin) = get_current_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    // Check if ShipHero is connected
    let Some(client) = state.shiphero() else {
        let template = NotConnectedTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: format!("/warehouse/shipments/{id}"),
        };
        return Html(template.render().unwrap_or_else(|e| {
            tracing::error!("Template render error: {}", e);
            "Internal Server Error".to_string()
        }))
        .into_response();
    };

    // Fetch shipment from ShipHero
    let result = client.get_shipment(&id).await;

    let (shipment, error_message) = match result {
        Ok(Some(shipment)) => (shipment, None),
        Ok(None) => {
            return Redirect::to("/warehouse/shipments?error=not_found").into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch ShipHero shipment");
            return Redirect::to(&format!(
                "/warehouse/shipments?error={}",
                urlencoding::encode(&e.to_string())
            ))
            .into_response();
        }
    };

    let template = WarehouseShipmentDetailTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: format!("/warehouse/shipments/{id}"),
        shipment,
        error_message,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

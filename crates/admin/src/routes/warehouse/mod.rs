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
    OrderConnection, OrderHistoryEvent, Shipment, ShipmentConnection, WarehouseOrder,
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

// =============================================================================
// Router
// =============================================================================

/// Build the warehouse router.
pub fn router() -> Router<AppState> {
    Router::new()
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

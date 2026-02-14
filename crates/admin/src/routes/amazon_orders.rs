//! Amazon orders routes.
//!
//! Provides a view of Amazon orders cached locally from the SP-API.
//! Only `super_admin` users can access these features.

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

use crate::db::{AmazonOrderRepository, CachedAmazonOrder, CachedAmazonOrderItem};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Amazon orders list page.
#[derive(Template)]
#[template(path = "amazon/orders/index.html")]
struct OrdersIndexTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
    orders: Vec<CachedAmazonOrder>,
    total_count: i64,
    page: i64,
    page_size: i64,
    status_filter: Option<String>,
    sync_error: Option<String>,
}

/// Amazon order detail page.
#[derive(Template)]
#[template(path = "amazon/orders/show.html")]
struct OrderShowTemplate {
    admin_user: AdminUserView,
    current_path: String,
    order: CachedAmazonOrder,
    items: Vec<CachedAmazonOrderItem>,
}

// =============================================================================
// Query Parameters
// =============================================================================

#[derive(Debug, Deserialize)]
struct OrdersQuery {
    page: Option<i64>,
    status: Option<String>,
}

// =============================================================================
// Router
// =============================================================================

/// Build the Amazon orders router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/amazon/orders", get(orders_index))
        .route("/amazon/orders/sync", get(orders_sync))
        .route("/amazon/orders/{id}", get(orders_show))
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /amazon/orders — Amazon orders list.
#[instrument(skip(state, session))]
async fn orders_index(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<OrdersQuery>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let connected = state.amazon().is_some();
    if !connected {
        return render(OrdersIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/amazon/orders".to_string(),
            connected: false,
            orders: vec![],
            total_count: 0,
            page: 1,
            page_size: 50,
            status_filter: None,
            sync_error: None,
        });
    }

    let page = query.page.unwrap_or(1).max(1);
    let page_size: i64 = 50;
    let offset = (page - 1) * page_size;
    let status_filter = query.status.as_deref();

    let repo = AmazonOrderRepository::new(state.pool());
    let orders = repo.list(page_size, offset, status_filter).await;
    let count = repo.count(status_filter).await;

    match (orders, count) {
        (Ok(orders), Ok(total_count)) => render(OrdersIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/amazon/orders".to_string(),
            connected: true,
            orders,
            total_count,
            page,
            page_size,
            status_filter: query.status,
            sync_error: None,
        }),
        (Err(e), _) | (_, Err(e)) => render(OrdersIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/amazon/orders".to_string(),
            connected: true,
            orders: vec![],
            total_count: 0,
            page: 1,
            page_size: 50,
            status_filter: None,
            sync_error: Some(format!("Failed to load orders: {e}")),
        }),
    }
}

/// GET /amazon/orders/sync — Trigger manual order sync from SP-API.
#[instrument(skip(state, session))]
async fn orders_sync(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(client) = state.amazon() else {
        return Redirect::to("/amazon/orders").into_response();
    };

    // Sync orders from the last 30 days
    let since = chrono::Utc::now() - chrono::Duration::days(30);
    let created_after = since.to_rfc3339();

    match client.get_all_orders_since(&created_after).await {
        Ok(orders) => {
            let repo = AmazonOrderRepository::new(state.pool());
            let mut synced = 0;
            for order in &orders {
                if upsert_order_from_api(&repo, order).await.is_ok() {
                    synced += 1;
                }
            }
            tracing::info!(count = synced, "Manual Amazon order sync complete");
        }
        Err(e) => {
            tracing::error!(error = %e, "Manual Amazon order sync failed");
        }
    }

    Redirect::to("/amazon/orders").into_response()
}

/// GET /amazon/orders/{id} — Amazon order detail.
#[instrument(skip(state, session))]
async fn orders_show(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let repo = AmazonOrderRepository::new(state.pool());
    let order = repo.get(&id).await;
    let items = repo.get_items(&id).await;

    match (order, items) {
        (Ok(Some(order)), Ok(items)) => render(OrderShowTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/amazon/orders".to_string(),
            order,
            items,
        }),
        _ => Redirect::to("/amazon/orders").into_response(),
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Upsert a single order from the SP-API response into the DB.
async fn upsert_order_from_api(
    repo: &AmazonOrderRepository<'_>,
    order: &naked_pineapple_services::amazon_sp::AmazonOrder,
) -> Result<(), crate::db::RepositoryError> {
    let purchase_date = order
        .purchase_date
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let last_update_date = order
        .last_update_date
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let raw_json = serde_json::to_value(order).ok();

    let params = crate::db::amazon_orders::UpsertOrderParams {
        amazon_order_id: &order.amazon_order_id,
        purchase_date,
        last_update_date,
        order_status: order.order_status.as_deref().unwrap_or("Unknown"),
        fulfillment_channel: order.fulfillment_channel.as_deref(),
        sales_channel: order.sales_channel.as_deref(),
        order_type: order.order_type.as_deref(),
        order_total_amount: order.order_total.as_ref().and_then(|m| m.amount.as_deref()),
        order_total_currency: order
            .order_total
            .as_ref()
            .and_then(|m| m.currency_code.as_deref()),
        number_of_items_shipped: order.number_of_items_shipped,
        number_of_items_unshipped: order.number_of_items_unshipped,
        is_business_order: order.is_business_order,
        is_prime: order.is_prime,
        marketplace_id: order.marketplace_id.as_deref(),
        ship_name: order
            .shipping_address
            .as_ref()
            .and_then(|a| a.name.as_deref()),
        ship_city: order
            .shipping_address
            .as_ref()
            .and_then(|a| a.city.as_deref()),
        ship_state: order
            .shipping_address
            .as_ref()
            .and_then(|a| a.state_or_region.as_deref()),
        ship_postal_code: order
            .shipping_address
            .as_ref()
            .and_then(|a| a.postal_code.as_deref()),
        ship_country: order
            .shipping_address
            .as_ref()
            .and_then(|a| a.country_code.as_deref()),
        buyer_email: order
            .buyer_info
            .as_ref()
            .and_then(|b| b.buyer_email.as_deref()),
        buyer_name: order
            .buyer_info
            .as_ref()
            .and_then(|b| b.buyer_name.as_deref()),
        raw_json: raw_json.as_ref(),
    };

    repo.upsert(&params).await?;
    Ok(())
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

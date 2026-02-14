//! Meta Commerce orders routes.
//!
//! Provides a view of Facebook Shop and Instagram Shopping orders
//! cached locally from the Graph API.
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

use crate::db::{CachedMetaOrder, CachedMetaOrderItem, MetaOrderRepository};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Meta orders list page.
#[derive(Template)]
#[template(path = "meta/orders/index.html")]
struct OrdersIndexTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
    orders: Vec<CachedMetaOrder>,
    total_count: i64,
    page: i64,
    page_size: i64,
    status_filter: Option<String>,
    channel_filter: Option<String>,
    sync_error: Option<String>,
}

/// Meta order detail page.
#[derive(Template)]
#[template(path = "meta/orders/show.html")]
struct OrderShowTemplate {
    admin_user: AdminUserView,
    current_path: String,
    order: CachedMetaOrder,
    items: Vec<CachedMetaOrderItem>,
}

// =============================================================================
// Query Parameters
// =============================================================================

#[derive(Debug, Deserialize)]
struct OrdersQuery {
    page: Option<i64>,
    status: Option<String>,
    channel: Option<String>,
}

// =============================================================================
// Router
// =============================================================================

/// Build the Meta orders router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/meta/orders", get(orders_index))
        .route("/meta/orders/sync", get(orders_sync))
        .route("/meta/orders/{id}", get(orders_show))
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /meta/orders — Meta orders list.
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

    let connected = state.meta().is_some();
    if !connected {
        return render(OrdersIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/meta/orders".to_string(),
            connected: false,
            orders: vec![],
            total_count: 0,
            page: 1,
            page_size: 50,
            status_filter: None,
            channel_filter: None,
            sync_error: None,
        });
    }

    let page = query.page.unwrap_or(1).max(1);
    let page_size: i64 = 50;
    let offset = (page - 1) * page_size;
    let status_filter = query.status.as_deref();
    let channel_filter = query.channel.as_deref();

    let repo = MetaOrderRepository::new(state.pool());
    let orders = repo
        .list(page_size, offset, status_filter, channel_filter)
        .await;
    let count = repo.count(status_filter, channel_filter).await;

    match (orders, count) {
        (Ok(orders), Ok(total_count)) => render(OrdersIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/meta/orders".to_string(),
            connected: true,
            orders,
            total_count,
            page,
            page_size,
            status_filter: query.status,
            channel_filter: query.channel,
            sync_error: None,
        }),
        (Err(e), _) | (_, Err(e)) => render(OrdersIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/meta/orders".to_string(),
            connected: true,
            orders: vec![],
            total_count: 0,
            page: 1,
            page_size: 50,
            status_filter: None,
            channel_filter: None,
            sync_error: Some(format!("Failed to load orders: {e}")),
        }),
    }
}

/// GET /meta/orders/sync — Trigger manual order sync from Graph API.
#[instrument(skip(state, session))]
async fn orders_sync(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(client) = state.meta() else {
        return Redirect::to("/meta/orders").into_response();
    };

    // Sync orders from the last 30 days
    let since = chrono::Utc::now() - chrono::Duration::days(30);
    let updated_after = since.to_rfc3339();

    match client.get_all_orders_since(&updated_after).await {
        Ok(orders) => {
            let repo = MetaOrderRepository::new(state.pool());
            let mut synced = 0;
            for order in &orders {
                if upsert_order_from_api(&repo, order).await.is_ok() {
                    synced += 1;
                }
            }
            tracing::info!(count = synced, "Manual Meta order sync complete");
        }
        Err(e) => {
            tracing::error!(error = %e, "Manual Meta order sync failed");
        }
    }

    Redirect::to("/meta/orders").into_response()
}

/// GET /meta/orders/{id} — Meta order detail.
#[instrument(skip(state, session))]
async fn orders_show(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i32>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let repo = MetaOrderRepository::new(state.pool());
    let order = repo.get_by_id(id).await;

    match order {
        Ok(Some(order)) => {
            let items = repo
                .get_items(&order.facebook_order_id)
                .await
                .unwrap_or_default();
            render(OrderShowTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/meta/orders".to_string(),
                order,
                items,
            })
        }
        _ => Redirect::to("/meta/orders").into_response(),
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Upsert a single order from the Graph API response into the DB.
async fn upsert_order_from_api(
    repo: &MetaOrderRepository<'_>,
    order: &naked_pineapple_services::meta_commerce::FacebookOrder,
) -> Result<(), crate::db::RepositoryError> {
    let created_time = order
        .created
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let last_updated_time = order
        .last_updated
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let raw_json = serde_json::to_value(order).ok();

    let order_status = order
        .order_status
        .as_ref()
        .and_then(|s| s.state.as_deref())
        .unwrap_or("CREATED");

    let channel = order.channel.as_deref().unwrap_or("facebook");

    let params = crate::db::meta_orders::UpsertMetaOrderParams {
        facebook_order_id: &order.id,
        created_time,
        last_updated_time,
        order_status,
        channel,
        buyer_name: order.buyer_details.as_ref().and_then(|b| b.name.as_deref()),
        buyer_email: order
            .buyer_details
            .as_ref()
            .and_then(|b| b.email.as_deref()),
        ship_name: order
            .shipping_address
            .as_ref()
            .and_then(|a| a.name.as_deref()),
        ship_street1: order
            .shipping_address
            .as_ref()
            .and_then(|a| a.street1.as_deref()),
        ship_street2: order
            .shipping_address
            .as_ref()
            .and_then(|a| a.street2.as_deref()),
        ship_city: order
            .shipping_address
            .as_ref()
            .and_then(|a| a.city.as_deref()),
        ship_state: order
            .shipping_address
            .as_ref()
            .and_then(|a| a.state.as_deref()),
        ship_postal_code: order
            .shipping_address
            .as_ref()
            .and_then(|a| a.postal_code.as_deref()),
        ship_country: order
            .shipping_address
            .as_ref()
            .and_then(|a| a.country.as_deref()),
        estimated_payment_amount: order
            .estimated_payment_details
            .as_ref()
            .and_then(|p| p.total_amount.as_ref())
            .and_then(|m| m.amount.as_deref()),
        estimated_payment_currency: order
            .estimated_payment_details
            .as_ref()
            .and_then(|p| p.total_amount.as_ref())
            .and_then(|m| m.currency.as_deref()),
        raw_json: raw_json.as_ref(),
    };

    repo.upsert(&params).await?;

    // Upsert order items
    if let Some(items_data) = &order.items {
        for item in &items_data.data {
            let item_params = crate::db::meta_orders::UpsertMetaOrderItemParams {
                facebook_order_id: &order.id,
                product_id: item.product_id.as_deref().unwrap_or("unknown"),
                retailer_id: item.retailer_id.as_deref(),
                quantity: item.quantity.unwrap_or(1),
                price_per_unit: item
                    .price_per_unit
                    .as_ref()
                    .and_then(|m| m.amount.as_deref()),
                currency: item
                    .price_per_unit
                    .as_ref()
                    .and_then(|m| m.currency.as_deref()),
            };
            let _ = repo.upsert_item(&item_params).await;
        }
    }

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

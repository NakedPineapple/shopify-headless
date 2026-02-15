//! TikTok Shop orders routes.
//!
//! Provides a view of TikTok Shop orders cached locally from
//! the TikTok Shop Open API. Only `super_admin` users can access
//! these features.

use askama::Template;
use axum::{
    Router,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use rust_decimal::Decimal;
use serde::Deserialize;
use tower_sessions::Session;
use tracing::instrument;

use crate::db::tiktok_orders::TikTokOrderFilters;
use crate::db::{CachedTikTokOrder, CachedTikTokOrderItem, TikTokOrderRepository};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// TikTok orders list page.
#[derive(Template)]
#[template(path = "tiktok/orders/index.html")]
struct OrdersIndexTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
    orders: Vec<CachedTikTokOrder>,
    total_count: i64,
    page: i64,
    page_size: i64,
    status_filter: Option<String>,
    source_filter: Option<String>,
    affiliate_filter: Option<bool>,
    sync_error: Option<String>,
}

/// TikTok order detail page.
#[derive(Template)]
#[template(path = "tiktok/orders/show.html")]
struct OrderShowTemplate {
    admin_user: AdminUserView,
    current_path: String,
    order: CachedTikTokOrder,
    items: Vec<CachedTikTokOrderItem>,
}

// =============================================================================
// Query Parameters
// =============================================================================

#[derive(Debug, Deserialize)]
struct OrdersQuery {
    page: Option<i64>,
    status: Option<String>,
    source_type: Option<String>,
    affiliate: Option<bool>,
}

// =============================================================================
// Router
// =============================================================================

/// Build the TikTok orders router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tiktok/orders", get(orders_index))
        .route("/tiktok/orders/sync", get(orders_sync))
        .route("/tiktok/orders/{id}", get(orders_show))
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /tiktok/orders -- TikTok orders list.
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

    let connected = state.tiktok().is_some();
    if !connected {
        return render(OrdersIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/tiktok/orders".to_string(),
            connected: false,
            orders: vec![],
            total_count: 0,
            page: 1,
            page_size: 50,
            status_filter: None,
            source_filter: None,
            affiliate_filter: None,
            sync_error: None,
        });
    }

    let page = query.page.unwrap_or(1).max(1);
    let page_size: i64 = 50;
    let offset = (page - 1) * page_size;

    let filters = TikTokOrderFilters {
        status: query.status.as_deref(),
        source: query.source_type.as_deref(),
        affiliate: query.affiliate,
    };

    let repo = TikTokOrderRepository::new(state.pool());
    let orders = repo.list(page_size, offset, &filters).await;
    let count = repo.count(&filters).await;

    match (orders, count) {
        (Ok(orders), Ok(total_count)) => render(OrdersIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/tiktok/orders".to_string(),
            connected: true,
            orders,
            total_count,
            page,
            page_size,
            status_filter: query.status,
            source_filter: query.source_type,
            affiliate_filter: query.affiliate,
            sync_error: None,
        }),
        (Err(e), _) | (_, Err(e)) => render(OrdersIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/tiktok/orders".to_string(),
            connected: true,
            orders: vec![],
            total_count: 0,
            page: 1,
            page_size: 50,
            status_filter: None,
            source_filter: None,
            affiliate_filter: None,
            sync_error: Some(format!("Failed to load orders: {e}")),
        }),
    }
}

/// GET /tiktok/orders/sync -- Trigger manual order sync from TikTok API.
#[instrument(skip(state, session))]
async fn orders_sync(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(client) = state.tiktok() else {
        return Redirect::to("/tiktok/orders").into_response();
    };

    // Sync orders from the last 30 days
    let since = chrono::Utc::now() - chrono::Duration::days(30);
    let since_timestamp = since.timestamp();

    match client.get_all_orders_since(since_timestamp).await {
        Ok(orders) => {
            let repo = TikTokOrderRepository::new(state.pool());
            let mut synced = 0;
            for order in &orders {
                if upsert_order_from_api(&repo, order).await.is_ok() {
                    synced += 1;
                }
            }
            tracing::info!(count = synced, "Manual TikTok order sync complete");
        }
        Err(e) => {
            tracing::error!(error = %e, "Manual TikTok order sync failed");
        }
    }

    Redirect::to("/tiktok/orders").into_response()
}

/// GET /tiktok/orders/{id} -- TikTok order detail.
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

    let repo = TikTokOrderRepository::new(state.pool());
    let order = repo.get_by_id(id).await;

    match order {
        Ok(Some(order)) => {
            let items = repo
                .get_items(&order.tiktok_order_id)
                .await
                .unwrap_or_default();
            render(OrderShowTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/tiktok/orders".to_string(),
                order,
                items,
            })
        }
        _ => Redirect::to("/tiktok/orders").into_response(),
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Upsert a single order from the TikTok API response into the DB.
async fn upsert_order_from_api(
    repo: &TikTokOrderRepository<'_>,
    order: &naked_pineapple_services::tiktok_shop::TikTokOrder,
) -> Result<(), crate::db::RepositoryError> {
    let created_time = order
        .create_time
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0));

    let last_updated_time = order
        .update_time
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0));

    let raw_json = serde_json::to_value(order).ok();
    let order_id = order.id.as_deref().unwrap_or("unknown");

    let params = build_upsert_params(order, created_time, last_updated_time, raw_json.as_ref());

    repo.upsert(&params).await?;

    // Upsert order items
    upsert_order_items(repo, order_id, order).await;

    Ok(())
}

/// Build upsert params from a TikTok API order.
fn build_upsert_params<'a>(
    order: &'a naked_pineapple_services::tiktok_shop::TikTokOrder,
    created_time: Option<chrono::DateTime<chrono::Utc>>,
    last_updated_time: Option<chrono::DateTime<chrono::Utc>>,
    raw_json: Option<&'a serde_json::Value>,
) -> crate::db::tiktok_orders::UpsertTikTokOrderParams<'a> {
    use crate::db::tiktok_orders::{
        UpsertTikTokOrderAffiliate, UpsertTikTokOrderFulfillment, UpsertTikTokOrderParams,
        UpsertTikTokOrderPayment, UpsertTikTokOrderShipping,
    };

    let commission_rate = order
        .commission
        .as_ref()
        .and_then(|c| c.rate.as_deref())
        .and_then(|r| r.parse::<Decimal>().ok());

    UpsertTikTokOrderParams {
        tiktok_order_id: order.id.as_deref().unwrap_or("unknown"),
        created_time,
        last_updated_time,
        order_status: order.status.as_deref().unwrap_or("UNKNOWN"),
        buyer_name: None,
        buyer_email: None,
        buyer_phone: order
            .recipient_address
            .as_ref()
            .and_then(|a| a.phone_number.as_deref()),
        shipping: UpsertTikTokOrderShipping {
            name: order
                .recipient_address
                .as_ref()
                .and_then(|a| a.name.as_deref()),
            street1: order
                .recipient_address
                .as_ref()
                .and_then(|a| a.address_line1.as_deref()),
            street2: order
                .recipient_address
                .as_ref()
                .and_then(|a| a.address_line2.as_deref()),
            city: order
                .recipient_address
                .as_ref()
                .and_then(|a| a.city.as_deref()),
            state: order
                .recipient_address
                .as_ref()
                .and_then(|a| a.state.as_deref()),
            postal_code: order
                .recipient_address
                .as_ref()
                .and_then(|a| a.zipcode.as_deref()),
            country: order
                .recipient_address
                .as_ref()
                .and_then(|a| a.country.as_deref()),
        },
        payment: UpsertTikTokOrderPayment {
            payment_amount: order
                .payment
                .as_ref()
                .and_then(|p| p.total_amount.as_deref()),
            payment_currency: order.payment.as_ref().and_then(|p| p.currency.as_deref()),
            shipping_amount: order
                .payment
                .as_ref()
                .and_then(|p| p.shipping_fee.as_deref()),
            platform_discount: order
                .payment
                .as_ref()
                .and_then(|p| p.platform_discount.as_deref()),
        },
        affiliate: UpsertTikTokOrderAffiliate {
            source_type: order.source_type.as_deref(),
            creator_username: order.creator.as_ref().and_then(|c| c.username.as_deref()),
            creator_id: order.creator.as_ref().and_then(|c| c.id.as_deref()),
            is_affiliate_order: order.is_affiliate_order.unwrap_or(false),
            commission_rate,
            commission_amount: order.commission.as_ref().and_then(|c| c.amount.as_deref()),
            commission_status: order.commission.as_ref().and_then(|c| c.status.as_deref()),
        },
        fulfillment: UpsertTikTokOrderFulfillment {
            is_fbt: order.fulfillment_type.as_deref() == Some("FBT"),
            fbt_warehouse_id: order.fbt_warehouse_id.as_deref(),
            shipping_provider_id: order
                .shipping
                .as_ref()
                .and_then(|s| s.provider_id.as_deref()),
            tracking_number: order
                .shipping
                .as_ref()
                .and_then(|s| s.tracking_number.as_deref()),
            shipping_status: order.shipping.as_ref().and_then(|s| s.status.as_deref()),
        },
        raw_json,
    }
}

/// Upsert line items for an order.
async fn upsert_order_items(
    repo: &TikTokOrderRepository<'_>,
    order_id: &str,
    order: &naked_pineapple_services::tiktok_shop::TikTokOrder,
) {
    if let Some(items) = &order.line_items {
        for item in items {
            let item_params = crate::db::tiktok_orders::UpsertTikTokOrderItemParams {
                tiktok_order_id: order_id,
                product_id: item.product_id.as_deref().unwrap_or("unknown"),
                sku_id: item.sku_id.as_deref(),
                product_name: item.product_name.as_deref(),
                quantity: item.quantity.unwrap_or(1),
                sale_price: item.sale_price.as_deref(),
                original_price: item.original_price.as_deref(),
                currency: item.currency.as_deref(),
                seller_discount: item.seller_discount.as_deref(),
                platform_discount: item.platform_discount.as_deref(),
            };
            let _ = repo.upsert_item(&item_params).await;
        }
    }
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

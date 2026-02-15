//! Faire wholesale orders routes.
//!
//! Provides a view of Faire wholesale orders cached locally from
//! the Faire Brand API v2. Only `super_admin` users can access
//! these features.

use askama::Template;
use axum::{
    Router,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tower_sessions::Session;
use tracing::instrument;

use crate::db::{CachedFaireOrder, CachedFaireOrderItem, FaireOrderRepository};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Faire orders list page.
#[derive(Template)]
#[template(path = "faire/orders/index.html")]
struct FaireOrdersTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
    orders: Vec<CachedFaireOrder>,
    total_count: i64,
    page: i64,
    page_size: i64,
    status_filter: Option<String>,
    sync_error: Option<String>,
}

/// Faire order detail page.
#[derive(Template)]
#[template(path = "faire/orders/show.html")]
struct FaireOrderDetailTemplate {
    admin_user: AdminUserView,
    current_path: String,
    order: CachedFaireOrder,
    items: Vec<CachedFaireOrderItem>,
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

/// Build the Faire orders router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/faire/orders", get(orders_index))
        .route("/faire/orders/sync", post(orders_sync))
        .route("/faire/orders/{id}", get(orders_show))
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /faire/orders -- Faire orders list.
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

    let connected = build_client_from_db(&state).await.is_some();
    if !connected {
        return render(FaireOrdersTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/faire/orders".to_string(),
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

    let repo = FaireOrderRepository::new(state.pool());
    let orders = repo.list(page_size, offset, status_filter).await;
    let count = repo.count(status_filter).await;

    match (orders, count) {
        (Ok(orders), Ok(total_count)) => render(FaireOrdersTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/faire/orders".to_string(),
            connected: true,
            orders,
            total_count,
            page,
            page_size,
            status_filter: query.status,
            sync_error: None,
        }),
        (Err(e), _) | (_, Err(e)) => render(FaireOrdersTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/faire/orders".to_string(),
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

/// POST /faire/orders/sync -- Trigger manual order sync from Faire API.
#[instrument(skip(state, session))]
async fn orders_sync(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(client) = build_client_from_db(&state).await else {
        return Redirect::to("/faire/orders").into_response();
    };

    // Sync orders from the last 30 days
    let since = chrono::Utc::now() - chrono::Duration::days(30);
    let since_str = since.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    match client.get_all_orders_since(&since_str).await {
        Ok(orders) => {
            let repo = FaireOrderRepository::new(state.pool());
            let mut synced = 0;
            for order in &orders {
                if upsert_order_from_api(&repo, order).await.is_ok() {
                    synced += 1;
                }
            }
            tracing::info!(count = synced, "Manual Faire order sync complete");
        }
        Err(e) => {
            tracing::error!(error = %e, "Manual Faire order sync failed");
        }
    }

    Redirect::to("/faire/orders").into_response()
}

/// GET /faire/orders/{id} -- Faire order detail.
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

    let repo = FaireOrderRepository::new(state.pool());
    let order = repo.get_by_id(id).await;

    match order {
        Ok(Some(order)) => {
            let items = repo
                .get_items(&order.faire_order_token)
                .await
                .unwrap_or_default();
            render(FaireOrderDetailTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/faire/orders".to_string(),
                order,
                items,
            })
        }
        _ => Redirect::to("/faire/orders").into_response(),
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Convert cents (i64) to a dollar string with two decimal places.
fn cents_to_dollars(cents: i64) -> String {
    #[allow(
        clippy::cast_precision_loss,
        reason = "monetary cent values will never approach 2^52"
    )]
    let dollars = cents as f64 / 100.0;
    format!("{dollars:.2}")
}

/// Upsert a single order from the Faire API response into the DB.
async fn upsert_order_from_api(
    repo: &FaireOrderRepository<'_>,
    order: &naked_pineapple_services::faire::FaireOrder,
) -> Result<(), crate::db::RepositoryError> {
    use crate::db::faire_orders::{
        UpsertFaireOrderFinancials, UpsertFaireOrderParams, UpsertFaireOrderRetailer,
        UpsertFaireOrderShipping,
    };

    let order_token = order.token.as_deref().unwrap_or("unknown");
    let raw_json = serde_json::to_value(order).ok();
    let costs = order.payout_costs.as_ref();
    let order_total = costs
        .and_then(|p| p.total_order_cents)
        .map(cents_to_dollars);
    let shipping_cost = costs.and_then(|p| p.shipping_cents).map(cents_to_dollars);
    let commission = costs.and_then(|p| p.commission_cents).map(cents_to_dollars);
    let net = costs
        .and_then(|p| p.total_payout_cents)
        .map(cents_to_dollars);

    let params = UpsertFaireOrderParams {
        faire_order_token: order_token,
        shopify_order_id: None,
        order_state: order.state.as_deref().unwrap_or("UNKNOWN"),
        created_at_faire: None,
        retailer: UpsertFaireOrderRetailer {
            id: order.retailer.as_ref().and_then(|r| r.token.as_deref()),
            name: order.retailer.as_ref().and_then(|r| r.name.as_deref()),
            email: order.retailer.as_ref().and_then(|r| r.email.as_deref()),
            phone: order.retailer.as_ref().and_then(|r| r.phone.as_deref()),
        },
        shipping: UpsertFaireOrderShipping {
            name: order.address.as_ref().and_then(|a| a.name.as_deref()),
            street1: order.address.as_ref().and_then(|a| a.address1.as_deref()),
            street2: order.address.as_ref().and_then(|a| a.address2.as_deref()),
            city: order.address.as_ref().and_then(|a| a.city.as_deref()),
            state: order.address.as_ref().and_then(|a| a.state.as_deref()),
            postal_code: order
                .address
                .as_ref()
                .and_then(|a| a.postal_code.as_deref()),
            country: order.address.as_ref().and_then(|a| a.country.as_deref()),
        },
        financials: UpsertFaireOrderFinancials {
            order_total: order_total.as_deref(),
            currency: None,
            shipping_cost: shipping_cost.as_deref(),
            faire_commission: commission.as_deref(),
            net_payout: net.as_deref(),
            is_first_order: false,
            payment_terms: None,
        },
        raw_json: raw_json.as_ref(),
    };
    repo.upsert(&params).await?;

    upsert_order_items(repo, order_token, order).await;
    Ok(())
}

/// Upsert line items for an order.
async fn upsert_order_items(
    repo: &FaireOrderRepository<'_>,
    order_token: &str,
    order: &naked_pineapple_services::faire::FaireOrder,
) {
    if let Some(items) = &order.items {
        for item in items {
            let item_params = crate::db::faire_orders::UpsertFaireOrderItemParams {
                faire_order_token: order_token,
                product_token: item.product_token.as_deref().unwrap_or("unknown"),
                product_option_token: item.product_option_token.as_deref(),
                product_name: item.product_name.as_deref(),
                quantity: item.quantity.unwrap_or(1),
                unit_price: None,
                total_price: None,
                sku: item.sku.as_deref(),
            };
            let _ = repo.upsert_item(&item_params).await;
        }
    }
}

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

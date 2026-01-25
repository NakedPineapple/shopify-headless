//! Orders list route handler.

use askama::Template;
use axum::{
    extract::{Query, State},
    response::Html,
};
use serde::Deserialize;
use tracing::instrument;

use crate::{
    filters,
    middleware::auth::RequireAdminAuth,
    models::CurrentAdmin,
    shopify::types::{FulfillmentStatus, Money, Order},
    state::AppState,
};

use naked_pineapple_core::AdminRole;

use super::dashboard::AdminUserView;

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub cursor: Option<String>,
    pub query: Option<String>,
    pub status: Option<String>,
}

/// Order view for templates.
#[derive(Debug, Clone)]
pub struct OrderView {
    pub id: String,
    pub name: String,
    pub customer_name: String,
    pub customer_email: Option<String>,
    pub total: String,
    pub status: String,
    pub status_class: String,
    pub created_at: String,
    pub item_count: i64,
}

// =============================================================================
// Type Conversions
// =============================================================================

/// Format a Shopify Money type as a price string.
fn format_price(money: &Money) -> String {
    if let Ok(amount) = money.amount.parse::<f64>() {
        format!("${amount:.2}")
    } else {
        format!("${}", money.amount)
    }
}

/// Get customer name from an order.
fn get_customer_name(order: &Order) -> String {
    if let Some(addr) = &order.shipping_address {
        let first = addr.first_name.as_deref().unwrap_or("");
        let last = addr.last_name.as_deref().unwrap_or("");
        let name = format!("{first} {last}").trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    if let Some(addr) = &order.billing_address {
        let first = addr.first_name.as_deref().unwrap_or("");
        let last = addr.last_name.as_deref().unwrap_or("");
        let name = format!("{first} {last}").trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    order.email.clone().unwrap_or_else(|| "Guest".to_string())
}

/// Map fulfillment status to display string and CSS class.
fn fulfillment_status_display(order: &Order) -> (String, String) {
    match order.fulfillment_status {
        Some(FulfillmentStatus::Fulfilled) => {
            ("Fulfilled".to_string(), "bg-green-100 text-green-700".to_string())
        }
        Some(FulfillmentStatus::PartiallyFulfilled) => {
            ("Partially Fulfilled".to_string(), "bg-blue-100 text-blue-700".to_string())
        }
        Some(FulfillmentStatus::Unfulfilled) | None => {
            ("Unfulfilled".to_string(), "bg-yellow-100 text-yellow-700".to_string())
        }
        Some(FulfillmentStatus::OnHold) => {
            ("On Hold".to_string(), "bg-red-100 text-red-700".to_string())
        }
        Some(FulfillmentStatus::InProgress) => {
            ("In Progress".to_string(), "bg-blue-100 text-blue-700".to_string())
        }
        _ => ("Pending".to_string(), "bg-gray-100 text-gray-700".to_string()),
    }
}

impl From<&Order> for OrderView {
    fn from(order: &Order) -> Self {
        let (status, status_class) = fulfillment_status_display(order);
        let item_count: i64 = order.line_items.iter().map(|li| li.quantity).sum();

        Self {
            id: order.id.clone(),
            name: order.name.clone(),
            customer_name: get_customer_name(order),
            customer_email: order.email.clone(),
            total: format_price(&order.total_price),
            status,
            status_class,
            created_at: order.created_at.clone(),
            item_count,
        }
    }
}

/// Orders list page template.
#[derive(Template)]
#[template(path = "orders/index.html")]
pub struct OrdersIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub orders: Vec<OrderView>,
    pub has_next_page: bool,
    pub next_cursor: Option<String>,
    pub search_query: Option<String>,
    pub status_filter: Option<String>,
}

/// Orders list page handler.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Html<String> {
    // Build query string with status filter if provided
    let search_query = match (&query.query, &query.status) {
        (Some(q), Some(s)) => Some(format!("{q} fulfillment_status:{s}")),
        (None, Some(s)) => Some(format!("fulfillment_status:{s}")),
        (Some(q), None) => Some(q.clone()),
        (None, None) => None,
    };

    let result = state
        .shopify()
        .get_orders(25, query.cursor.clone(), search_query)
        .await;

    let (orders, has_next_page, next_cursor) = match result {
        Ok(conn) => {
            let orders: Vec<OrderView> = conn.orders.iter().map(OrderView::from).collect();
            (
                orders,
                conn.page_info.has_next_page,
                conn.page_info.end_cursor,
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch orders: {e}");
            (vec![], false, None)
        }
    };

    let template = OrdersIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/orders".to_string(),
        orders,
        has_next_page,
        next_cursor,
        search_query: query.query,
        status_filter: query.status,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

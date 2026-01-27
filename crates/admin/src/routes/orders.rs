//! Orders management route handlers.

use askama::Template;
use axum::{
    Form,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use serde::Deserialize;
use tracing::instrument;

use crate::{
    filters,
    middleware::auth::RequireAdminAuth,
    shopify::types::{
        Address, FinancialStatus, Fulfillment, FulfillmentStatus, Money, Order, OrderLineItem,
        TrackingInfo,
    },
    state::AppState,
};

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
    money.amount.parse::<f64>().map_or_else(
        |_| format!("${}", money.amount),
        |amount| format!("${amount:.2}"),
    )
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
        Some(FulfillmentStatus::Fulfilled) => (
            "Fulfilled".to_string(),
            "bg-green-100 text-green-700".to_string(),
        ),
        Some(FulfillmentStatus::PartiallyFulfilled) => (
            "Partially Fulfilled".to_string(),
            "bg-blue-100 text-blue-700".to_string(),
        ),
        Some(FulfillmentStatus::Unfulfilled) | None => (
            "Unfulfilled".to_string(),
            "bg-yellow-100 text-yellow-700".to_string(),
        ),
        Some(FulfillmentStatus::OnHold) => {
            ("On Hold".to_string(), "bg-red-100 text-red-700".to_string())
        }
        Some(FulfillmentStatus::InProgress) => (
            "In Progress".to_string(),
            "bg-blue-100 text-blue-700".to_string(),
        ),
        _ => (
            "Pending".to_string(),
            "bg-gray-100 text-gray-700".to_string(),
        ),
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

// =============================================================================
// Order Detail Views
// =============================================================================

/// Line item view for templates.
#[derive(Debug, Clone)]
pub struct LineItemView {
    pub title: String,
    pub variant_title: Option<String>,
    pub sku: Option<String>,
    pub quantity: i64,
    pub unit_price: String,
    pub total_price: String,
}

// Precision loss from i64 -> f64 is acceptable here since order quantities
// are realistically bounded (never approaching 2^53) and this is for display only.
#[allow(clippy::cast_precision_loss)]
impl From<&OrderLineItem> for LineItemView {
    fn from(item: &OrderLineItem) -> Self {
        let unit_price = format_price(&item.discounted_unit_price);
        let total = item
            .discounted_unit_price
            .amount
            .parse::<f64>()
            .unwrap_or(0.0)
            * item.quantity as f64;
        Self {
            title: item.title.clone(),
            variant_title: item.variant_title.clone(),
            sku: item.sku.clone(),
            quantity: item.quantity,
            unit_price,
            total_price: format!("${total:.2}"),
        }
    }
}

/// Address view for templates.
#[derive(Debug, Clone)]
pub struct AddressView {
    pub name: String,
    pub line1: String,
    pub line2: Option<String>,
    pub city_state_zip: String,
    pub country: String,
    pub phone: Option<String>,
}

impl From<&Address> for AddressView {
    fn from(addr: &Address) -> Self {
        let first = addr.first_name.as_deref().unwrap_or("");
        let last = addr.last_name.as_deref().unwrap_or("");
        let name = format!("{first} {last}").trim().to_string();

        let line1 = addr.address1.clone().unwrap_or_default();
        let line2 = addr.address2.clone().filter(|s| !s.is_empty());

        let city = addr.city.as_deref().unwrap_or("");
        let state = addr.province_code.as_deref().unwrap_or("");
        let zip = addr.zip.as_deref().unwrap_or("");
        let city_state_zip = format!("{city}, {state} {zip}").trim().to_string();

        let country = addr.country_code.clone().unwrap_or_default();

        Self {
            name: if name.is_empty() {
                "N/A".to_string()
            } else {
                name
            },
            line1,
            line2,
            city_state_zip,
            country,
            phone: addr.phone.clone(),
        }
    }
}

/// Fulfillment view for templates.
#[derive(Debug, Clone)]
pub struct FulfillmentView {
    pub id: String,
    pub status: String,
    pub tracking_number: Option<String>,
    pub tracking_url: Option<String>,
    pub carrier: Option<String>,
    pub created_at: String,
}

impl From<&Fulfillment> for FulfillmentView {
    fn from(f: &Fulfillment) -> Self {
        let tracking = f.tracking_info.first();
        Self {
            id: f.id.clone(),
            status: f.status.clone(),
            tracking_number: tracking.and_then(|t| t.number.clone()),
            tracking_url: tracking.and_then(|t| t.url.clone()),
            carrier: tracking.and_then(|t| t.company.clone()),
            created_at: f.created_at.clone(),
        }
    }
}

/// Order detail view for templates.
#[derive(Debug, Clone)]
pub struct OrderDetailView {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub customer_name: String,
    pub customer_email: Option<String>,
    pub customer_phone: Option<String>,
    pub note: Option<String>,
    pub fulfillment_status: String,
    pub fulfillment_status_class: String,
    pub financial_status: String,
    pub financial_status_class: String,
    pub is_paid: bool,
    pub is_test: bool,
    pub subtotal: String,
    pub shipping: String,
    pub tax: String,
    pub discount: String,
    pub total: String,
    pub line_items: Vec<LineItemView>,
    pub fulfillments: Vec<FulfillmentView>,
    pub shipping_address: Option<AddressView>,
    pub billing_address: Option<AddressView>,
}

/// Map financial status to display string and CSS class.
fn financial_status_display(order: &Order) -> (String, String, bool) {
    match order.financial_status {
        Some(FinancialStatus::Paid) => (
            "Paid".to_string(),
            "bg-green-100 text-green-700".to_string(),
            true,
        ),
        Some(FinancialStatus::Authorized) => (
            "Authorized".to_string(),
            "bg-blue-100 text-blue-700".to_string(),
            false,
        ),
        Some(FinancialStatus::Pending) => (
            "Pending".to_string(),
            "bg-yellow-100 text-yellow-700".to_string(),
            false,
        ),
        Some(FinancialStatus::PartiallyPaid) => (
            "Partially Paid".to_string(),
            "bg-yellow-100 text-yellow-700".to_string(),
            false,
        ),
        Some(FinancialStatus::Refunded) => (
            "Refunded".to_string(),
            "bg-gray-100 text-gray-700".to_string(),
            true,
        ),
        Some(FinancialStatus::PartiallyRefunded) => (
            "Partially Refunded".to_string(),
            "bg-gray-100 text-gray-700".to_string(),
            true,
        ),
        Some(FinancialStatus::Voided) => (
            "Voided".to_string(),
            "bg-red-100 text-red-700".to_string(),
            false,
        ),
        _ => (
            "Unknown".to_string(),
            "bg-gray-100 text-gray-700".to_string(),
            false,
        ),
    }
}

impl From<&Order> for OrderDetailView {
    fn from(order: &Order) -> Self {
        let (fulfillment_status, fulfillment_status_class) = fulfillment_status_display(order);
        let (financial_status, financial_status_class, is_paid) = financial_status_display(order);

        Self {
            id: order.id.clone(),
            name: order.name.clone(),
            created_at: order.created_at.clone(),
            customer_name: get_customer_name(order),
            customer_email: order.email.clone(),
            customer_phone: order.phone.clone(),
            note: order.note.clone(),
            fulfillment_status,
            fulfillment_status_class,
            financial_status,
            financial_status_class,
            is_paid,
            is_test: order.test,
            subtotal: format_price(&order.subtotal_price),
            shipping: format_price(&order.total_shipping_price),
            tax: format_price(&order.total_tax),
            discount: format_price(&order.total_discounts),
            total: format_price(&order.total_price),
            line_items: order.line_items.iter().map(LineItemView::from).collect(),
            fulfillments: order
                .fulfillments
                .iter()
                .map(FulfillmentView::from)
                .collect(),
            shipping_address: order.shipping_address.as_ref().map(AddressView::from),
            billing_address: order.billing_address.as_ref().map(AddressView::from),
        }
    }
}

/// Order detail page template.
#[derive(Template)]
#[template(path = "orders/show.html")]
pub struct OrderShowTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub order: OrderDetailView,
    pub error: Option<String>,
}

/// Note update form input.
#[derive(Debug, Deserialize)]
pub struct NoteFormInput {
    pub note: Option<String>,
}

/// Cancel form input.
#[derive(Debug, Deserialize)]
pub struct CancelFormInput {
    pub reason: Option<String>,
    pub notify_customer: Option<String>,
    pub refund: Option<String>,
    pub restock: Option<String>,
}

/// Order detail page handler.
#[instrument(skip(admin, state))]
pub async fn show(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state.shopify().get_order(&order_id).await {
        Ok(Some(order)) => {
            let template = OrderShowTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/orders".to_string(),
                order: OrderDetailView::from(&order),
                error: None,
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Order not found").into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch order: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch order").into_response()
        }
    }
}

/// Update order note handler.
#[instrument(skip(admin, state))]
pub async fn update_note(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<NoteFormInput>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state
        .shopify()
        .update_order_note(&order_id, input.note.as_deref())
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %order_id, "Order note updated");
            // Redirect back to order detail
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to update order note");
            // Fetch order again and show error
            match state.shopify().get_order(&order_id).await {
                Ok(Some(order)) => {
                    let template = OrderShowTemplate {
                        admin_user: AdminUserView::from(&admin),
                        current_path: "/orders".to_string(),
                        order: OrderDetailView::from(&order),
                        error: Some(format!("Failed to update note: {e}")),
                    };
                    Html(template.render().unwrap_or_else(|e| {
                        tracing::error!("Template render error: {}", e);
                        "Internal Server Error".to_string()
                    }))
                    .into_response()
                }
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to update order").into_response(),
            }
        }
    }
}

/// Mark order as paid handler.
#[instrument(skip(_admin, state))]
pub async fn mark_paid(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state.shopify().mark_order_as_paid(&order_id).await {
        Ok(()) => {
            tracing::info!(order_id = %order_id, "Order marked as paid");
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to mark order as paid");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to mark as paid: {e}"),
            )
                .into_response()
        }
    }
}

/// Cancel order handler.
#[instrument(skip(_admin, state))]
pub async fn cancel(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<CancelFormInput>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    let notify = input.notify_customer.as_deref() == Some("on");
    let refund = input.refund.as_deref() == Some("on");
    let restock = input.restock.as_deref() == Some("on");

    match state
        .shopify()
        .cancel_order(&order_id, input.reason.as_deref(), notify, refund, restock)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %order_id, reason = ?input.reason, "Order cancelled");
            Redirect::to("/orders").into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to cancel order");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to cancel order: {e}"),
            )
                .into_response()
        }
    }
}

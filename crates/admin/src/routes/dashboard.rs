//! Dashboard route handler.

use askama::Template;
use axum::{extract::State, response::Html};
use tracing::instrument;

use crate::{
    filters,
    middleware::auth::RequireAdminAuth,
    models::CurrentAdmin,
    shopify::types::{Money, Order},
    state::AppState,
};

use naked_pineapple_core::AdminRole;

/// Admin user view for templates.
#[derive(Debug, Clone)]
pub struct AdminUserView {
    pub name: String,
    pub email: String,
    pub is_super_admin: bool,
}

impl From<&CurrentAdmin> for AdminUserView {
    fn from(admin: &CurrentAdmin) -> Self {
        Self {
            name: admin.name.clone(),
            email: admin.email.to_string(),
            is_super_admin: admin.role == AdminRole::SuperAdmin,
        }
    }
}

/// Dashboard metrics.
#[derive(Debug, Clone)]
pub struct DashboardMetrics {
    pub orders: String,
    pub revenue: String,
    pub customers: String,
    pub products: String,
}

impl Default for DashboardMetrics {
    fn default() -> Self {
        Self {
            orders: "0".to_string(),
            revenue: "$0.00".to_string(),
            customers: "0".to_string(),
            products: "0".to_string(),
        }
    }
}

/// Recent order view for dashboard.
#[derive(Debug, Clone)]
pub struct RecentOrderView {
    pub number: String,
    pub customer_name: String,
    pub total: String,
    pub status: String,
}

/// Activity item for dashboard.
#[derive(Debug, Clone)]
pub struct ActivityView {
    pub activity_type: String,
    pub icon: String,
    pub description: String,
    pub time_ago: String,
}

/// Dashboard template.
#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub metrics: DashboardMetrics,
    pub recent_orders: Vec<RecentOrderView>,
    pub recent_activity: Vec<ActivityView>,
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
    // Try shipping address first, then billing address
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
    // Fall back to email
    order.email.clone().unwrap_or_else(|| "Guest".to_string())
}

/// Map fulfillment status to display string.
fn fulfillment_status_display(order: &Order) -> String {
    match order.fulfillment_status {
        Some(crate::shopify::types::FulfillmentStatus::Fulfilled) => "Fulfilled".to_string(),
        Some(crate::shopify::types::FulfillmentStatus::PartiallyFulfilled) => {
            "Partially Fulfilled".to_string()
        }
        Some(crate::shopify::types::FulfillmentStatus::Unfulfilled) | None => {
            "Unfulfilled".to_string()
        }
        Some(crate::shopify::types::FulfillmentStatus::OnHold) => "On Hold".to_string(),
        Some(crate::shopify::types::FulfillmentStatus::InProgress) => "In Progress".to_string(),
        _ => "Pending".to_string(),
    }
}

impl From<&Order> for RecentOrderView {
    fn from(order: &Order) -> Self {
        Self {
            number: order.name.clone(),
            customer_name: get_customer_name(order),
            total: format_price(&order.total_price),
            status: fulfillment_status_display(order),
        }
    }
}

/// Dashboard page handler.
#[instrument(skip(admin, state))]
pub async fn dashboard(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
) -> Html<String> {
    // Fetch data from Shopify Admin API in parallel
    let orders_future = state.shopify().get_orders(50, None, None);
    let products_future = state.shopify().get_products(1, None, None);
    let customers_future = state.shopify().get_customers(1, None, None);

    let (orders_result, products_result, customers_result) =
        tokio::join!(orders_future, products_future, customers_future);

    // Process orders for metrics and recent orders
    let (order_count, total_revenue, recent_orders) = match orders_result {
        Ok(order_conn) => {
            let count = order_conn.orders.len();
            let revenue: f64 = order_conn
                .orders
                .iter()
                .filter_map(|o| o.total_price.amount.parse::<f64>().ok())
                .sum();
            let recent: Vec<RecentOrderView> = order_conn
                .orders
                .iter()
                .take(5)
                .map(RecentOrderView::from)
                .collect();
            (count, revenue, recent)
        }
        Err(e) => {
            tracing::error!("Failed to fetch orders: {e}");
            (0, 0.0, vec![])
        }
    };

    // Get product count (from page info if available, else use results)
    let product_count = match products_result {
        Ok(product_conn) => {
            // Shopify doesn't return total count easily, so we approximate
            // In production, you'd cache this or use a separate count query
            if product_conn.page_info.has_next_page {
                "50+".to_string() // Indicates there are more
            } else {
                product_conn.products.len().to_string()
            }
        }
        Err(e) => {
            tracing::error!("Failed to fetch products: {e}");
            "0".to_string()
        }
    };

    // Get customer count
    let customer_count = match customers_result {
        Ok(customer_conn) => {
            if customer_conn.page_info.has_next_page {
                "50+".to_string()
            } else {
                customer_conn.customers.len().to_string()
            }
        }
        Err(e) => {
            tracing::error!("Failed to fetch customers: {e}");
            "0".to_string()
        }
    };

    let metrics = DashboardMetrics {
        orders: order_count.to_string(),
        revenue: format!("${total_revenue:.2}"),
        customers: customer_count,
        products: product_count,
    };

    // Build activity feed from recent orders
    let recent_activity: Vec<ActivityView> = recent_orders
        .iter()
        .take(5)
        .map(|order| ActivityView {
            activity_type: "order".to_string(),
            icon: "📦".to_string(),
            description: format!("New order {} from {}", order.number, order.customer_name),
            time_ago: "Recently".to_string(), // Would need proper time formatting
        })
        .collect();

    let template = DashboardTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/".to_string(),
        metrics,
        recent_orders,
        recent_activity,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

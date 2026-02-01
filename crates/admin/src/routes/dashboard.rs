//! Dashboard route handler.

use askama::Template;
use axum::{extract::State, response::Html};
use tracing::{debug, info, instrument, warn};

use crate::{
    filters,
    middleware::auth::RequireAdminAuth,
    models::CurrentAdmin,
    shopify::types::{AdminProduct, Money, Order, ProductStatus},
    state::AppState,
};

use naked_pineapple_core::AdminRole;

/// Low stock threshold for alerts.
const LOW_STOCK_THRESHOLD: i64 = 10;

/// Admin user view for templates.
#[derive(Debug, Clone)]
pub struct AdminUserView {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub is_super_admin: bool,
}

impl From<&CurrentAdmin> for AdminUserView {
    fn from(admin: &CurrentAdmin) -> Self {
        Self {
            id: admin.id.as_i32(),
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

/// Low stock item view for dashboard alerts.
#[derive(Debug, Clone)]
pub struct LowStockItemView {
    pub product_title: String,
    pub variant_title: String,
    pub sku: Option<String>,
    pub quantity: i64,
    pub image_url: Option<String>,
}

impl LowStockItemView {
    fn from_product(product: &AdminProduct) -> Vec<Self> {
        // Only include active products
        if product.status != ProductStatus::Active {
            return vec![];
        }

        product
            .variants
            .iter()
            .filter(|v| v.inventory_quantity <= LOW_STOCK_THRESHOLD)
            .map(|v| Self {
                product_title: product.title.clone(),
                variant_title: v.title.clone(),
                sku: v.sku.clone(),
                quantity: v.inventory_quantity,
                image_url: product.featured_image.as_ref().map(|img| img.url.clone()),
            })
            .collect()
    }
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
    pub low_stock_items: Vec<LowStockItemView>,
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

/// Process orders result into metrics and recent orders list.
fn process_orders_result(
    result: Result<crate::shopify::types::OrderConnection, crate::shopify::AdminShopifyError>,
) -> (usize, f64, Vec<RecentOrderView>) {
    match result {
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
            debug!(
                order_count = count,
                total_revenue = revenue,
                "Fetched orders from Shopify"
            );
            (count, revenue, recent)
        }
        Err(e) => {
            warn!(error = %e, "Failed to fetch orders from Shopify");
            (0, 0.0, vec![])
        }
    }
}

/// Process products result into count and low stock items.
fn process_products_result(
    result: Result<
        crate::shopify::types::AdminProductConnection,
        crate::shopify::AdminShopifyError,
    >,
) -> (String, Vec<LowStockItemView>) {
    match result {
        Ok(product_conn) => {
            let count = if product_conn.page_info.has_next_page {
                "50+".to_string()
            } else {
                product_conn.products.len().to_string()
            };
            let low_stock: Vec<LowStockItemView> = product_conn
                .products
                .iter()
                .flat_map(LowStockItemView::from_product)
                .take(5)
                .collect();
            debug!(product_count = %count, "Fetched products from Shopify");
            if !low_stock.is_empty() {
                info!(low_stock_count = low_stock.len(), "Found low stock items");
            }
            (count, low_stock)
        }
        Err(e) => {
            warn!(error = %e, "Failed to fetch products from Shopify");
            ("0".to_string(), vec![])
        }
    }
}

/// Process customers result into count string.
fn process_customers_result(
    result: Result<crate::shopify::types::CustomerConnection, crate::shopify::AdminShopifyError>,
) -> String {
    match result {
        Ok(customer_conn) => {
            let count = if customer_conn.page_info.has_next_page {
                "50+".to_string()
            } else {
                customer_conn.customers.len().to_string()
            };
            debug!(customer_count = %count, "Fetched customers from Shopify");
            count
        }
        Err(e) => {
            warn!(error = %e, "Failed to fetch customers from Shopify");
            "0".to_string()
        }
    }
}

/// Dashboard page handler.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn dashboard(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
) -> Html<String> {
    debug!("Loading dashboard page");

    // Fetch data from Shopify Admin API in parallel
    let orders_future = state.shopify().get_orders(50, None, None);
    let products_future = state.shopify().get_products(50, None, None);
    let customers_future =
        state
            .shopify()
            .get_customers(crate::shopify::types::CustomerListParams {
                first: Some(1),
                ..Default::default()
            });

    let (orders_result, products_result, customers_result) =
        tokio::join!(orders_future, products_future, customers_future);

    let (order_count, total_revenue, recent_orders) = process_orders_result(orders_result);
    let (product_count, low_stock_items) = process_products_result(products_result);
    let customer_count = process_customers_result(customers_result);

    let metrics = DashboardMetrics {
        orders: order_count.to_string(),
        revenue: format!("${total_revenue:.2}"),
        customers: customer_count,
        products: product_count,
    };

    let recent_activity: Vec<ActivityView> = recent_orders
        .iter()
        .take(5)
        .map(|order| ActivityView {
            activity_type: "order".to_string(),
            icon: "📦".to_string(),
            description: format!("New order {} from {}", order.number, order.customer_name),
            time_ago: "Recently".to_string(),
        })
        .collect();

    let template = DashboardTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/".to_string(),
        metrics,
        recent_orders,
        recent_activity,
        low_stock_items,
    };

    Html(template.render().unwrap_or_else(|e| {
        warn!(error = %e, "Dashboard template render error");
        "Internal Server Error".to_string()
    }))
}

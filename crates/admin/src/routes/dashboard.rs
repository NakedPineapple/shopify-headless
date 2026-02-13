//! Dashboard route handler.

use askama::Template;
use axum::{extract::State, response::Html};
use tracing::{debug, info, instrument, warn};

use crate::{
    db::{ExpenseRepository, InventoryLotRepository},
    filters,
    middleware::auth::RequireAdminAuth,
    models::CurrentAdmin,
    shopify::types::{
        AdminProduct, AnalyticsSummary, DailyMetrics, DateRange, Money, Order, ProductStatus,
    },
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
    pub expenses: String,
    pub net_income: String,
    pub order_growth: GrowthIndicator,
    pub revenue_growth: GrowthIndicator,
}

impl Default for DashboardMetrics {
    fn default() -> Self {
        Self {
            orders: "0".to_string(),
            revenue: "$0.00".to_string(),
            customers: "0".to_string(),
            products: "0".to_string(),
            expenses: "$0.00".to_string(),
            net_income: "$0.00".to_string(),
            order_growth: GrowthIndicator::default(),
            revenue_growth: GrowthIndicator::default(),
        }
    }
}

/// Growth indicator for period-over-period comparison.
#[derive(Debug, Clone, Default)]
pub struct GrowthIndicator {
    /// Formatted percentage string (e.g., "+12%", "-5%", "0%").
    pub label: String,
    /// CSS class for coloring: "text-success", "text-destructive", or "text-muted-foreground".
    pub css_class: String,
    /// Icon name: "trend-up", "trend-down", or "minus".
    pub icon: String,
}

impl GrowthIndicator {
    fn compute(current: f64, previous: f64) -> Self {
        if previous == 0.0 {
            return Self {
                label: if current > 0.0 {
                    "New".to_string()
                } else {
                    "0%".to_string()
                },
                css_class: "text-muted-foreground".to_string(),
                icon: "minus".to_string(),
            };
        }
        let pct = ((current - previous) / previous) * 100.0;
        if pct > 0.5 {
            Self {
                label: format!("+{pct:.0}%"),
                css_class: "text-success".to_string(),
                icon: "trend-up".to_string(),
            }
        } else if pct < -0.5 {
            Self {
                label: format!("{pct:.0}%"),
                css_class: "text-destructive".to_string(),
                icon: "trend-down".to_string(),
            }
        } else {
            Self {
                label: "0%".to_string(),
                css_class: "text-muted-foreground".to_string(),
                icon: "minus".to_string(),
            }
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
    pub revenue_trend_labels: String,
    pub revenue_trend_data: String,
    pub channel_labels: String,
    pub channel_data: String,
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

/// Format daily trend data as JSON arrays for Chart.js.
fn format_trend_data(trend: &[DailyMetrics]) -> (String, String) {
    let labels: Vec<String> = trend
        .iter()
        .map(|d| {
            // Convert "2026-01-15" → "Jan 15"
            chrono::NaiveDate::parse_from_str(&d.date, "%Y-%m-%d")
                .map_or_else(|_| d.date.clone(), |dt| dt.format("%b %d").to_string())
        })
        .collect();
    let data: Vec<String> = trend
        .iter()
        .map(|d| format!("{:.2}", d.total_sales))
        .collect();
    (
        serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_string()),
        format!("[{}]", data.join(",")),
    )
}

/// Format channel summary data as JSON arrays for Chart.js.
fn format_channel_data(summary: &AnalyticsSummary) -> (String, String) {
    let mut sorted: Vec<&crate::shopify::types::ChannelMetrics> = summary.channels.iter().collect();
    sorted.sort_by(|a, b| {
        b.total_sales
            .partial_cmp(&a.total_sales)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let top = sorted.into_iter().take(5);
    let labels: Vec<String> = top.clone().map(|c| c.channel_name.clone()).collect();
    let data: Vec<String> = top.map(|c| format!("{:.2}", c.total_sales)).collect();
    (
        serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_string()),
        format!("[{}]", data.join(",")),
    )
}

/// Fetch expense and cost data for the dashboard.
async fn fetch_cost_data(pool: &sqlx::PgPool) -> (rust_decimal::Decimal, rust_decimal::Decimal) {
    let now = chrono::Utc::now().date_naive();
    let start = now - chrono::Duration::days(30);
    let expense_repo = ExpenseRepository::new(pool);
    let lot_repo = InventoryLotRepository::new(pool);

    let (expenses, cogs) = tokio::join!(
        expense_repo.get_total_expenses(start, now),
        lot_repo.get_total_cogs(start, now),
    );

    (expenses.unwrap_or_default(), cogs.unwrap_or_default())
}

/// Build metrics from Shopify data, analytics, and costs.
// Order counts in e-commerce will never exceed i64's f64-safe range (2^52)
#[allow(clippy::cast_precision_loss)]
fn build_metrics(
    order_count: usize,
    total_revenue: f64,
    customer_count: String,
    product_count: String,
    current_analytics: &AnalyticsSummary,
    prev_analytics: &AnalyticsSummary,
    total_expenses: rust_decimal::Decimal,
) -> DashboardMetrics {
    let net = total_revenue - total_expenses.to_string().parse::<f64>().unwrap_or(0.0);

    DashboardMetrics {
        orders: order_count.to_string(),
        revenue: format!("${total_revenue:.2}"),
        customers: customer_count,
        products: product_count,
        expenses: format!("${total_expenses:.2}"),
        net_income: format!("${net:.2}"),
        order_growth: GrowthIndicator::compute(
            current_analytics.total_orders as f64,
            prev_analytics.total_orders as f64,
        ),
        revenue_growth: GrowthIndicator::compute(
            current_analytics.total_sales,
            prev_analytics.total_sales,
        ),
    }
}

/// Dashboard page handler.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn dashboard(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
) -> Html<String> {
    debug!("Loading dashboard page");

    let current_range = DateRange::default();
    let prev_range = DateRange {
        start: "-60d".to_string(),
        end: "-31d".to_string(),
    };

    // Fetch all data in parallel
    let (orders_result, products_result, customers_result, analytics, prev_analytics, trend, costs) = tokio::join!(
        state.shopify().get_orders(50, None, None),
        state.shopify().get_products(50, None, None),
        state
            .shopify()
            .get_customers(crate::shopify::types::CustomerListParams {
                first: Some(1),
                ..Default::default()
            }),
        state.shopify().get_channel_analytics(&current_range),
        state.shopify().get_channel_analytics(&prev_range),
        state.shopify().get_channel_trend(None, &current_range),
        fetch_cost_data(state.pool()),
    );

    let (order_count, total_revenue, recent_orders) = process_orders_result(orders_result);
    let (product_count, low_stock_items) = process_products_result(products_result);
    let customer_count = process_customers_result(customers_result);
    let current_summary = analytics.unwrap_or_default();
    let prev_summary = prev_analytics.unwrap_or_default();
    let daily_trend = trend.unwrap_or_default();
    let (operating_expenses, cogs) = costs;
    let total_expenses = operating_expenses + cogs;

    let metrics = build_metrics(
        order_count,
        total_revenue,
        customer_count,
        product_count,
        &current_summary,
        &prev_summary,
        total_expenses,
    );

    let (revenue_trend_labels, revenue_trend_data) = format_trend_data(&daily_trend);
    let (channel_labels, channel_data) = format_channel_data(&current_summary);

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
        revenue_trend_labels,
        revenue_trend_data,
        channel_labels,
        channel_data,
    };

    Html(template.render().unwrap_or_else(|e| {
        warn!(error = %e, "Dashboard template render error");
        "Internal Server Error".to_string()
    }))
}

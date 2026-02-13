//! Financials route handlers.
//!
//! Handles manufacturing batches, inventory lots, and cost tracking.

use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use tracing::{debug, info, instrument, warn};

use crate::{
    db::{ExpenseRepository, InventoryLotRepository, ManufacturingRepository, RepositoryError},
    filters,
    middleware::auth::RequireAdminAuth,
    models::expense::ExpenseFilter,
    models::inventory_lot::{CreateLotInput, UpdateLotInput},
    models::manufacturing::{BatchFilter, CreateBatchInput, UpdateBatchInput},
    shopify::types::DateRange,
    state::AppState,
};

use super::dashboard::AdminUserView;

// =============================================================================
// Query Parameters
// =============================================================================

/// Query parameters for batch list.
#[derive(Debug, Deserialize)]
pub struct BatchesQuery {
    pub product_id: Option<String>,
    pub batch_number: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub page: Option<i64>,
}

/// Query parameters for lot list.
#[derive(Debug, Deserialize)]
pub struct LotsQuery {
    pub has_remaining: Option<bool>,
    pub page: Option<i64>,
}

// =============================================================================
// Form Inputs
// =============================================================================

/// Form data for creating a batch.
#[derive(Debug, Deserialize)]
pub struct CreateBatchForm {
    pub batch_number: String,
    pub shopify_product_id: String,
    pub shopify_variant_id: Option<String>,
    pub quantity: i32,
    pub manufacture_date: NaiveDate,
    pub raw_cost_per_item: Decimal,
    pub label_cost_per_item: Decimal,
    pub outer_carton_cost_per_item: Decimal,
    pub currency_code: Option<String>,
    pub notes: Option<String>,
}

/// Form data for updating a batch.
#[derive(Debug, Deserialize)]
pub struct UpdateBatchForm {
    pub batch_number: Option<String>,
    pub quantity: Option<i32>,
    pub manufacture_date: Option<NaiveDate>,
    pub raw_cost_per_item: Option<Decimal>,
    pub label_cost_per_item: Option<Decimal>,
    pub outer_carton_cost_per_item: Option<Decimal>,
    pub currency_code: Option<String>,
    pub notes: Option<String>,
}

/// Form data for creating a lot.
#[derive(Debug, Deserialize)]
pub struct CreateLotForm {
    pub lot_number: String,
    pub quantity: i32,
    pub received_date: NaiveDate,
    pub shopify_location_id: Option<String>,
    pub notes: Option<String>,
}

/// Form data for updating a lot.
#[derive(Debug, Deserialize)]
pub struct UpdateLotForm {
    pub lot_number: Option<String>,
    pub quantity: Option<i32>,
    pub received_date: Option<NaiveDate>,
    pub shopify_location_id: Option<String>,
    pub notes: Option<String>,
}

// =============================================================================
// View Types
// =============================================================================

/// Batch view for templates.
#[derive(Debug, Clone)]
pub struct BatchView {
    pub id: i32,
    pub batch_number: String,
    pub shopify_product_id: String,
    pub shopify_variant_id: Option<String>,
    /// Product title from Shopify.
    pub product_title: Option<String>,
    /// Product image URL from Shopify.
    pub product_image: Option<String>,
    /// Variant title from Shopify (if variant selected).
    pub variant_title: Option<String>,
    /// Short numeric product ID for links.
    pub product_short_id: String,
    pub quantity: i32,
    pub manufacture_date: String,
    pub raw_cost_per_item: String,
    pub label_cost_per_item: String,
    pub outer_carton_cost_per_item: String,
    pub cost_per_unit: String,
    pub total_batch_cost: String,
    pub currency_code: String,
    pub notes: Option<String>,
    pub lots_received: i64,
}

/// Lot view for templates.
#[derive(Debug, Clone)]
pub struct LotView {
    pub id: i32,
    pub lot_number: String,
    pub quantity: i32,
    pub quantity_remaining: i64,
    pub received_date: String,
    pub shopify_location_id: Option<String>,
    /// Location name from Shopify.
    pub location_name: Option<String>,
    /// Short numeric location ID for display.
    pub location_short_id: Option<String>,
    pub notes: Option<String>,
}

/// Location view for dropdown selection.
#[derive(Debug, Clone)]
pub struct LocationView {
    pub id: String,
    pub name: String,
    pub is_active: bool,
}

/// Product search result for picker.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProductSearchResult {
    pub id: String,
    pub short_id: String,
    pub title: String,
    pub image_url: Option<String>,
    pub variants: Vec<VariantSearchResult>,
}

/// Variant search result for picker.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VariantSearchResult {
    pub id: String,
    pub short_id: String,
    pub title: String,
    pub sku: Option<String>,
    pub price: String,
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Extract numeric ID from Shopify GID.
fn extract_short_id(gid: &str) -> String {
    gid.split('/').next_back().unwrap_or(gid).to_string()
}

/// Fetch product info from Shopify and create `BatchView` with product details.
async fn enrich_batch_view(
    shopify: &crate::shopify::AdminClient,
    batch: &crate::models::manufacturing::ManufacturingBatchWithDetails,
) -> BatchView {
    let mut product_title = None;
    let mut product_image = None;
    let mut variant_title = None;

    // Try to fetch product info from Shopify
    if let Ok(Some(product)) = shopify.get_product(&batch.batch.shopify_product_id).await {
        product_title = Some(product.title.clone());
        product_image = product.images.first().map(|img| img.url.clone());

        // If variant is specified, find its title
        if let Some(ref variant_id) = batch.batch.shopify_variant_id {
            variant_title = product
                .variants
                .iter()
                .find(|v| &v.id == variant_id)
                .map(|v| v.title.clone());
        }
    }

    BatchView {
        id: batch.batch.id.as_i32(),
        batch_number: batch.batch.batch_number.clone(),
        shopify_product_id: batch.batch.shopify_product_id.clone(),
        shopify_variant_id: batch.batch.shopify_variant_id.clone(),
        product_title,
        product_image,
        variant_title,
        product_short_id: extract_short_id(&batch.batch.shopify_product_id),
        quantity: batch.batch.quantity,
        manufacture_date: batch.batch.manufacture_date.format("%Y-%m-%d").to_string(),
        raw_cost_per_item: format!("{:.4}", batch.batch.raw_cost_per_item),
        label_cost_per_item: format!("{:.4}", batch.batch.label_cost_per_item),
        outer_carton_cost_per_item: format!("{:.4}", batch.batch.outer_carton_cost_per_item),
        cost_per_unit: format!("{:.4}", batch.batch.cost_per_unit),
        total_batch_cost: format!("{:.2}", batch.batch.total_batch_cost),
        currency_code: batch.batch.currency_code.clone(),
        notes: batch.batch.notes.clone(),
        lots_received: batch.lots_received,
    }
}

/// Create a simple `BatchView` without Shopify enrichment (for error cases).
fn simple_batch_view(
    batch: &crate::models::manufacturing::ManufacturingBatchWithDetails,
) -> BatchView {
    BatchView {
        id: batch.batch.id.as_i32(),
        batch_number: batch.batch.batch_number.clone(),
        shopify_product_id: batch.batch.shopify_product_id.clone(),
        shopify_variant_id: batch.batch.shopify_variant_id.clone(),
        product_title: None,
        product_image: None,
        variant_title: None,
        product_short_id: extract_short_id(&batch.batch.shopify_product_id),
        quantity: batch.batch.quantity,
        manufacture_date: batch.batch.manufacture_date.format("%Y-%m-%d").to_string(),
        raw_cost_per_item: format!("{:.4}", batch.batch.raw_cost_per_item),
        label_cost_per_item: format!("{:.4}", batch.batch.label_cost_per_item),
        outer_carton_cost_per_item: format!("{:.4}", batch.batch.outer_carton_cost_per_item),
        cost_per_unit: format!("{:.4}", batch.batch.cost_per_unit),
        total_batch_cost: format!("{:.2}", batch.batch.total_batch_cost),
        currency_code: batch.batch.currency_code.clone(),
        notes: batch.batch.notes.clone(),
        lots_received: batch.lots_received,
    }
}

/// Fetch locations from Shopify.
async fn fetch_locations(shopify: &crate::shopify::AdminClient) -> Vec<LocationView> {
    match shopify.get_locations().await {
        Ok(connection) => connection
            .locations
            .into_iter()
            .filter(|l| l.is_active)
            .map(|l| LocationView {
                id: l.id,
                name: l.name,
                is_active: l.is_active,
            })
            .collect(),
        Err(e) => {
            tracing::warn!(?e, "Failed to fetch locations from Shopify");
            vec![]
        }
    }
}

/// Find location name by ID.
fn find_location_name(locations: &[LocationView], location_id: &str) -> Option<String> {
    locations
        .iter()
        .find(|l| l.id == location_id)
        .map(|l| l.name.clone())
}

/// Build a single `LotView` from lot data.
fn build_lot_view(
    lwr: &crate::models::inventory_lot::InventoryLotWithRemaining,
    locations: &[LocationView],
) -> LotView {
    let location_name = lwr
        .lot
        .shopify_location_id
        .as_ref()
        .and_then(|lid| find_location_name(locations, lid));
    let location_short_id = lwr
        .lot
        .shopify_location_id
        .as_ref()
        .map(|lid| extract_short_id(lid));
    LotView {
        id: lwr.lot.id.as_i32(),
        lot_number: lwr.lot.lot_number.clone(),
        quantity: lwr.lot.quantity,
        quantity_remaining: lwr.quantity_remaining,
        received_date: lwr.lot.received_date.to_string(),
        shopify_location_id: lwr.lot.shopify_location_id.clone(),
        location_name,
        location_short_id,
        notes: lwr.lot.notes.clone(),
    }
}

/// Convert lots with remaining quantity to lot views.
fn lots_to_views(
    lots: &[crate::models::inventory_lot::InventoryLotWithRemaining],
    locations: &[LocationView],
) -> Vec<LotView> {
    lots.iter()
        .map(|lwr| build_lot_view(lwr, locations))
        .collect()
}

/// Fetch product info (title, image, variant title) from Shopify.
async fn fetch_product_info(
    shopify: &crate::shopify::AdminClient,
    product_id: &str,
    variant_id: Option<&str>,
) -> (Option<String>, Option<String>, Option<String>) {
    match shopify.get_product(product_id).await {
        Ok(Some(product)) => {
            let title = Some(product.title.clone());
            let image = product.images.first().map(|img| img.url.clone());
            let var_title = variant_id.and_then(|vid| {
                product
                    .variants
                    .iter()
                    .find(|v| v.id == vid)
                    .map(|v| v.title.clone())
            });
            (title, image, var_title)
        }
        Ok(None) | Err(_) => (None, None, None),
    }
}

/// Convert products to search results.
fn products_to_search_results(
    products: Vec<crate::shopify::types::AdminProduct>,
) -> Vec<ProductSearchResult> {
    products
        .into_iter()
        .map(|p| ProductSearchResult {
            short_id: extract_short_id(&p.id),
            id: p.id,
            title: p.title,
            image_url: p.images.first().map(|img| img.url.clone()),
            variants: p
                .variants
                .into_iter()
                .map(|v| VariantSearchResult {
                    short_id: extract_short_id(&v.id),
                    id: v.id,
                    title: v.title,
                    sku: v.sku,
                    price: format!("${}", v.price.amount),
                })
                .collect(),
        })
        .collect()
}

// =============================================================================
// Templates
// =============================================================================

/// Manufacturing batches index page.
#[derive(Template)]
#[template(path = "financials/manufacturing/index.html")]
struct ManufacturingIndexTemplate {
    admin_user: AdminUserView,
    current_path: String,
    batches: Vec<BatchView>,
    query: BatchesQuery,
    page: i64,
    total_count: i64,
    has_next: bool,
    has_prev: bool,
}

/// New batch form page.
#[derive(Template)]
#[template(path = "financials/manufacturing/new.html")]
struct ManufacturingNewTemplate {
    admin_user: AdminUserView,
    current_path: String,
    /// Recent products for quick selection.
    recent_products: Vec<ProductSearchResult>,
}

/// Batch detail page.
#[derive(Template)]
#[template(path = "financials/manufacturing/show.html")]
struct ManufacturingShowTemplate {
    admin_user: AdminUserView,
    current_path: String,
    batch: BatchView,
    lots: Vec<LotView>,
}

/// Edit batch form page.
#[derive(Template)]
#[template(path = "financials/manufacturing/edit.html")]
struct ManufacturingEditTemplate {
    admin_user: AdminUserView,
    current_path: String,
    batch: BatchView,
}

/// New lot form page.
#[derive(Template)]
#[template(path = "financials/manufacturing/lots/new.html")]
struct LotNewTemplate {
    admin_user: AdminUserView,
    current_path: String,
    batch: BatchView,
    /// Available locations for dropdown.
    locations: Vec<LocationView>,
}

/// Edit lot form page.
#[derive(Template)]
#[template(path = "financials/manufacturing/lots/edit.html")]
struct LotEditTemplate {
    admin_user: AdminUserView,
    current_path: String,
    batch: BatchView,
    lot: LotView,
    /// Available locations for dropdown.
    locations: Vec<LocationView>,
}

/// Product search results template (HTMX partial).
#[derive(Template)]
#[template(path = "financials/manufacturing/_product_search_results.html")]
struct ProductSearchResultsTemplate {
    products: Vec<ProductSearchResult>,
}

/// Financials overview page.
#[derive(Template)]
#[template(path = "financials/overview.html")]
struct FinancialsOverviewTemplate {
    admin_user: AdminUserView,
    current_path: String,
    manufacturing_cost: String,
    operating_expenses: String,
    total_costs: String,
    batch_count: i64,
    expense_count: i64,
    recent_expenses: Vec<RecentExpenseView>,
    period_label: String,
}

/// Compact expense view for overview page.
#[derive(Debug, Clone)]
struct RecentExpenseView {
    description: String,
    amount: String,
    date: String,
    category_name: String,
}

// =============================================================================
// Margin Types and Template
// =============================================================================

/// Query parameters for margins page.
#[derive(Debug, Deserialize)]
pub struct MarginsQuery {
    pub range: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
}

impl MarginsQuery {
    fn to_date_range(&self) -> DateRange {
        if let (Some(start), Some(end)) = (&self.start, &self.end) {
            return DateRange::new(start.clone(), end.clone());
        }
        match self.range.as_deref() {
            Some("7d") => DateRange::last_days(7),
            Some("90d") => DateRange::last_days(90),
            Some("ytd") => DateRange::new("-1y", "today"),
            _ => DateRange::last_days(30),
        }
    }

    fn current_range(&self) -> &str {
        self.range.as_deref().unwrap_or("30d")
    }

    fn to_dates(&self) -> (NaiveDate, NaiveDate) {
        let now = chrono::Utc::now().date_naive();
        if let (Some(s), Some(e)) = (&self.start, &self.end) {
            let start = NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .unwrap_or(now - chrono::Duration::days(30));
            let end = NaiveDate::parse_from_str(e, "%Y-%m-%d").unwrap_or(now);
            (start, end)
        } else {
            let days = match self.range.as_deref() {
                Some("7d") => 7,
                Some("90d") => 90,
                Some("ytd") => 365,
                _ => 30,
            };
            (now - chrono::Duration::days(days), now)
        }
    }
}

/// Per-product margin view for templates.
#[derive(Debug, Clone)]
pub struct ProductMarginView {
    pub product_title: String,
    pub image_url: Option<String>,
    pub revenue: String,
    pub revenue_raw: f64,
    pub cogs: String,
    pub cogs_raw: f64,
    pub gross_profit: String,
    pub gross_margin_pct: String,
    pub margin_class: &'static str,
    pub units_sold: i64,
    pub units_allocated: i64,
    pub coverage_pct: String,
    pub has_cogs: bool,
}

/// Per-order margin view for the recent orders table.
#[derive(Debug, Clone)]
pub struct OrderMarginView {
    pub order_name: String,
    pub order_date: String,
    pub order_total: String,
    pub order_total_raw: f64,
    pub cogs: String,
    pub cogs_raw: f64,
    pub gross_profit: String,
    pub gross_margin_pct: String,
    pub margin_class: &'static str,
}

/// Business-wide margin summary view.
#[derive(Debug, Clone)]
pub struct MarginSummaryView {
    pub total_revenue: String,
    pub total_cogs: String,
    pub total_operating_expenses: String,
    pub gross_profit: String,
    pub net_profit: String,
    pub gross_margin_pct: String,
    pub net_margin_pct: String,
    pub allocation_coverage_pct: String,
}

/// Margins page template.
#[derive(Template)]
#[template(path = "financials/margins/index.html")]
struct MarginsTemplate {
    admin_user: AdminUserView,
    current_path: String,
    summary: MarginSummaryView,
    products: Vec<ProductMarginView>,
    recent_orders: Vec<OrderMarginView>,
    current_range: String,
    custom_start: String,
    custom_end: String,
    top_products_labels: String,
    top_products_data: String,
    waterfall_data: String,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// Financials overview page — financial health hub.
#[instrument(skip(state))]
pub async fn index(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let pool = state.pool();
    let now = chrono::Utc::now().date_naive();
    let start = now - chrono::Duration::days(30);

    let expense_repo = ExpenseRepository::new(pool);
    let manufacturing_repo = ManufacturingRepository::new(pool);
    let lot_repo = InventoryLotRepository::new(pool);

    let batch_filter = BatchFilter::default();
    let expense_filter = ExpenseFilter::default();
    let recent_filter = ExpenseFilter {
        limit: Some(5),
        ..Default::default()
    };

    let (operating_total, mfg_cost, batch_count, expense_count, recent) = tokio::join!(
        expense_repo.get_total_expenses(start, now),
        lot_repo.get_total_cogs(start, now),
        manufacturing_repo.count_batches(&batch_filter),
        expense_repo.count_expenses(&expense_filter),
        expense_repo.list_expenses(&recent_filter),
    );

    let operating = operating_total.unwrap_or_default();
    let mfg = mfg_cost.unwrap_or_default();
    let total = operating + mfg;

    let recent_expenses: Vec<RecentExpenseView> = recent
        .unwrap_or_default()
        .iter()
        .map(|e| RecentExpenseView {
            description: e.expense.description.clone(),
            amount: format!("${:.2}", e.expense.amount),
            date: e.expense.date.format("%b %d").to_string(),
            category_name: e.category_name.clone(),
        })
        .collect();

    let template = FinancialsOverviewTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials".to_string(),
        manufacturing_cost: format!("${mfg:.2}"),
        operating_expenses: format!("${operating:.2}"),
        total_costs: format!("${total:.2}"),
        batch_count: batch_count.unwrap_or(0),
        expense_count: expense_count.unwrap_or(0),
        recent_expenses,
        period_label: format!("{} — {}", start.format("%b %d"), now.format("%b %d, %Y")),
    };
    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
}

/// Profit margins reporting page.
#[instrument(skip(state), fields(admin_id = %user.id.as_i32()))]
pub async fn margins(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Query(query): Query<MarginsQuery>,
) -> impl IntoResponse {
    debug!("Fetching profit margins page");
    let date_range = query.to_date_range();
    let (start, end) = query.to_dates();

    let lot_repo = InventoryLotRepository::new(state.pool());
    let expense_repo = ExpenseRepository::new(state.pool());

    // Parallel fetch: COGS by product, total COGS, expenses, revenue, recent orders, products
    let (cogs_by_product, total_cogs, total_expenses, revenue_result, recent_cogs, shopify_prods) = tokio::join!(
        lot_repo.get_cogs_by_product(start, end),
        lot_repo.get_total_cogs(start, end),
        expense_repo.get_total_expenses(start, end),
        state.shopify().get_revenue_by_product(&date_range),
        lot_repo.get_recent_order_cogs(start, end, 10),
        state.shopify().get_products(50, None, None),
    );

    let cogs_map = build_cogs_map(cogs_by_product.unwrap_or_default());
    let total_cogs_val = total_cogs
        .unwrap_or_default()
        .to_string()
        .parse::<f64>()
        .unwrap_or(0.0);
    let total_expenses_val = total_expenses
        .unwrap_or_default()
        .to_string()
        .parse::<f64>()
        .unwrap_or(0.0);
    let revenue_products = revenue_result.unwrap_or_default();
    let recent_order_cogs = recent_cogs.unwrap_or_default();

    // Build product image map: title -> first image URL
    let image_map = build_product_image_map(&shopify_prods);

    let (products, total_revenue, total_units_sold, total_units_allocated) =
        build_product_margins(&revenue_products, &cogs_map, &image_map);

    let gross_profit = total_revenue - total_cogs_val;
    let net_profit = total_revenue - total_cogs_val - total_expenses_val;
    let gross_margin = margin_pct(gross_profit, total_revenue);
    let net_margin = margin_pct(net_profit, total_revenue);
    #[allow(clippy::cast_precision_loss)] // unit counts are always small enough for f64
    let coverage = if total_units_sold > 0 {
        (total_units_allocated as f64 / total_units_sold as f64 * 100.0).min(100.0)
    } else {
        0.0
    };

    let (top_labels, top_data) = build_top_products_chart(&products);

    // Build recent orders with margins by fetching order details from Shopify
    let recent_orders = build_recent_order_margins(state.shopify(), &recent_order_cogs).await;

    // Build waterfall chart data: [revenue, cogs, opex, net_profit]
    let waterfall_data = serde_json::to_string(&[
        total_revenue,
        total_cogs_val,
        total_expenses_val,
        net_profit,
    ])
    .unwrap_or_else(|_| "[0,0,0,0]".to_string());

    let summary = MarginSummaryView {
        total_revenue: format_margin_currency(total_revenue),
        total_cogs: format_margin_currency(total_cogs_val),
        total_operating_expenses: format_margin_currency(total_expenses_val),
        gross_profit: format_margin_currency(gross_profit),
        net_profit: format_margin_currency(net_profit),
        gross_margin_pct: format!("{gross_margin:.1}%"),
        net_margin_pct: format!("{net_margin:.1}%"),
        allocation_coverage_pct: format!("{coverage:.0}%"),
    };

    let template = MarginsTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/margins".to_string(),
        summary,
        products,
        recent_orders,
        current_range: query.current_range().to_string(),
        custom_start: query.start.clone().unwrap_or_default(),
        custom_end: query.end.clone().unwrap_or_default(),
        top_products_labels: top_labels,
        top_products_data: top_data,
        waterfall_data,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// Build a map of product title -> (`total_cogs`, `units_allocated`).
///
/// Note: `ShopifyQL` returns product titles, but COGS are keyed by product ID.
/// Since we can't directly join on title<->ID here, we use a simplified approach:
/// the COGS map is keyed by product ID. We'll match what we can.
fn build_cogs_map(
    cogs: Vec<crate::db::inventory_lot::ProductCogs>,
) -> std::collections::HashMap<String, (f64, i64)> {
    cogs.into_iter()
        .map(|c| {
            let val = c.total_cogs.to_string().parse::<f64>().unwrap_or(0.0);
            (c.shopify_product_id, (val, c.units_allocated))
        })
        .collect()
}

/// Build a map of product title -> first image URL from Shopify products.
fn build_product_image_map(
    shopify_result: &Result<
        crate::shopify::types::AdminProductConnection,
        crate::shopify::AdminShopifyError,
    >,
) -> std::collections::HashMap<String, String> {
    shopify_result.as_ref().map_or_else(
        |_| std::collections::HashMap::new(),
        |conn| {
            conn.products
                .iter()
                .filter_map(|p| {
                    p.images
                        .first()
                        .map(|img| (p.title.clone(), img.url.clone()))
                })
                .collect()
        },
    )
}

/// Build per-product margin views from revenue data, COGS map, and image map.
fn build_product_margins(
    revenue_products: &[crate::shopify::types::ProductRevenue],
    cogs_map: &std::collections::HashMap<String, (f64, i64)>,
    image_map: &std::collections::HashMap<String, String>,
) -> (Vec<ProductMarginView>, f64, i64, i64) {
    let mut total_revenue = 0.0;
    let mut total_units_sold: i64 = 0;
    let mut total_units_allocated: i64 = 0;

    // For now, aggregate total COGS across all products (we can't join by title)
    let total_cogs_from_map: f64 = cogs_map.values().map(|(c, _)| c).sum();
    let total_allocated_from_map: i64 = cogs_map.values().map(|(_, a)| a).sum();

    let mut products: Vec<ProductMarginView> = revenue_products
        .iter()
        .map(|p| {
            total_revenue += p.total_sales;
            total_units_sold += p.units_sold;

            // Try to match by product_id if available
            let (cogs_val, units_allocated) = p
                .product_id
                .as_ref()
                .and_then(|id| cogs_map.get(id))
                .copied()
                .unwrap_or((0.0, 0));

            total_units_allocated += units_allocated;
            let gross_profit = p.total_sales - cogs_val;
            let margin = margin_pct(gross_profit, p.total_sales);
            #[allow(clippy::cast_precision_loss)] // unit counts fit in f64
            let coverage = if p.units_sold > 0 {
                (units_allocated as f64 / p.units_sold as f64 * 100.0).min(100.0)
            } else {
                0.0
            };

            ProductMarginView {
                product_title: p.product_title.clone(),
                image_url: image_map.get(&p.product_title).cloned(),
                revenue: format_margin_currency(p.total_sales),
                revenue_raw: p.total_sales,
                cogs: if cogs_val > 0.0 {
                    format_margin_currency(cogs_val)
                } else {
                    "\u{2014}".to_string()
                },
                cogs_raw: cogs_val,
                gross_profit: format_margin_currency(gross_profit),
                gross_margin_pct: if cogs_val > 0.0 {
                    format!("{margin:.1}%")
                } else {
                    "\u{2014}".to_string()
                },
                margin_class: margin_color_class(margin, cogs_val > 0.0),
                units_sold: p.units_sold,
                units_allocated,
                coverage_pct: if cogs_val > 0.0 {
                    format!("{coverage:.0}%")
                } else {
                    "\u{2014}".to_string()
                },
                has_cogs: cogs_val > 0.0,
            }
        })
        .collect();

    // Sort by revenue descending
    products.sort_by(|a, b| {
        b.revenue_raw
            .partial_cmp(&a.revenue_raw)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Update total_units_allocated from the map for business-wide coverage
    if total_units_allocated == 0 {
        total_units_allocated = total_allocated_from_map;
    }
    // Use total_cogs_from_map for business-wide summary if per-product didn't match
    let _ = total_cogs_from_map;

    (
        products,
        total_revenue,
        total_units_sold,
        total_units_allocated,
    )
}

/// Build order margin views by fetching order details from Shopify.
async fn build_recent_order_margins(
    shopify: &crate::shopify::AdminClient,
    recent_cogs: &[crate::db::inventory_lot::RecentOrderCogs],
) -> Vec<OrderMarginView> {
    if recent_cogs.is_empty() {
        return vec![];
    }

    // Fetch all orders in parallel
    let order_futures: Vec<_> = recent_cogs
        .iter()
        .map(|c| shopify.get_order(&c.shopify_order_id))
        .collect();
    let order_results = futures::future::join_all(order_futures).await;

    recent_cogs
        .iter()
        .zip(order_results)
        .filter_map(|(cogs, order_result)| {
            let order = order_result.ok().flatten()?;
            let total: f64 = order.total_price.amount.parse().unwrap_or(0.0);
            let cogs_val: f64 = cogs.total_cogs.to_string().parse().unwrap_or(0.0);
            let profit = total - cogs_val;
            let margin = margin_pct(profit, total);

            Some(OrderMarginView {
                order_name: order.name,
                order_date: order.created_at[..10].to_string(),
                order_total: format_margin_currency(total),
                order_total_raw: total,
                cogs: format_margin_currency(cogs_val),
                cogs_raw: cogs_val,
                gross_profit: format_margin_currency(profit),
                gross_margin_pct: format!("{margin:.1}%"),
                margin_class: margin_color_class(margin, true),
            })
        })
        .collect()
}

/// Calculate margin percentage.
fn margin_pct(profit: f64, revenue: f64) -> f64 {
    if revenue > 0.0 {
        (profit / revenue) * 100.0
    } else {
        0.0
    }
}

/// CSS class for margin color coding.
fn margin_color_class(margin: f64, has_cogs: bool) -> &'static str {
    if !has_cogs {
        return "text-muted-foreground";
    }
    if margin >= 40.0 {
        "text-green-500"
    } else if margin >= 20.0 {
        "text-yellow-500"
    } else {
        "text-red-500"
    }
}

/// Build chart data for top products by gross profit.
fn build_top_products_chart(products: &[ProductMarginView]) -> (String, String) {
    let top: Vec<&ProductMarginView> = products.iter().filter(|p| p.has_cogs).take(10).collect();

    if top.is_empty() {
        return ("[]".to_string(), "[]".to_string());
    }

    let labels: Vec<&str> = top.iter().map(|p| p.product_title.as_str()).collect();
    let data: Vec<String> = top
        .iter()
        .map(|p| format!("{:.2}", p.revenue_raw - p.cogs_raw))
        .collect();

    let labels_json = serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_string());
    let data_json = format!("[{}]", data.join(","));
    (labels_json, data_json)
}

/// Format currency for margins display.
fn format_margin_currency(amount: f64) -> String {
    if amount >= 1_000_000.0 {
        format!("${:.2}M", amount / 1_000_000.0)
    } else if amount >= 1_000.0 {
        format!("${:.2}K", amount / 1_000.0)
    } else if amount <= -1_000.0 {
        format!("-${:.2}K", (-amount) / 1_000.0)
    } else {
        format!("${amount:.2}")
    }
}

/// Manufacturing batches index.
#[instrument(skip(state), fields(admin_id = %user.id.as_i32()))]
pub async fn manufacturing_index(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Query(query): Query<BatchesQuery>,
) -> impl IntoResponse {
    debug!("Listing manufacturing batches with filters");
    let repo = ManufacturingRepository::new(state.pool());

    let page = query.page.unwrap_or(1).max(1);
    let limit = 25_i64;
    let offset = (page - 1) * limit;

    let filter = BatchFilter {
        shopify_product_id: query.product_id.clone(),
        batch_number: query.batch_number.clone(),
        start_date: query.start_date,
        end_date: query.end_date,
        limit: Some(limit),
        offset: Some(offset),
    };

    let batches = match repo.list_batches(&filter).await {
        Ok(batches) => {
            debug!(count = batches.len(), "Retrieved batches from database");
            batches
        }
        Err(e) => {
            tracing::error!(?e, "Failed to list batches");
            return Html(format!("Error: {e}")).into_response();
        }
    };

    let total_count = match repo.count_batches(&filter).await {
        Ok(count) => count,
        Err(e) => {
            tracing::error!(?e, "Failed to count batches");
            0
        }
    };

    // Convert to views with lots_received counts and product info
    let mut batch_views = Vec::with_capacity(batches.len());
    for batch in &batches {
        let lots_received = repo.get_lots_received(batch.id).await.unwrap_or(0);

        // Try to fetch product info from Shopify
        let (product_title, product_image, variant_title) = if let Ok(Some(product)) =
            state.shopify().get_product(&batch.shopify_product_id).await
        {
            let title = Some(product.title.clone());
            let image = product.images.first().map(|img| img.url.clone());
            let var_title = batch.shopify_variant_id.as_ref().and_then(|vid| {
                product
                    .variants
                    .iter()
                    .find(|v| &v.id == vid)
                    .map(|v| v.title.clone())
            });
            (title, image, var_title)
        } else {
            (None, None, None)
        };

        batch_views.push(BatchView {
            id: batch.id.as_i32(),
            batch_number: batch.batch_number.clone(),
            shopify_product_id: batch.shopify_product_id.clone(),
            shopify_variant_id: batch.shopify_variant_id.clone(),
            product_title,
            product_image,
            variant_title,
            product_short_id: extract_short_id(&batch.shopify_product_id),
            quantity: batch.quantity,
            manufacture_date: batch.manufacture_date.to_string(),
            raw_cost_per_item: format!("{:.4}", batch.raw_cost_per_item),
            label_cost_per_item: format!("{:.4}", batch.label_cost_per_item),
            outer_carton_cost_per_item: format!("{:.4}", batch.outer_carton_cost_per_item),
            cost_per_unit: format!("{:.4}", batch.cost_per_unit),
            total_batch_cost: format!("{:.2}", batch.total_batch_cost),
            currency_code: batch.currency_code.clone(),
            notes: batch.notes.clone(),
            lots_received,
        });
    }

    let has_next = (page * limit) < total_count;
    let has_prev = page > 1;

    let template = ManufacturingIndexTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/manufacturing".to_string(),
        batches: batch_views,
        query,
        page,
        total_count,
        has_next,
        has_prev,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// New batch form.
#[instrument(skip(state), fields(admin_id = %user.id.as_i32()))]
pub async fn manufacturing_new(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
) -> impl IntoResponse {
    debug!("Rendering new batch form");
    // Fetch recent products for quick selection
    let recent_products = match state.shopify().get_products(10, None, None).await {
        Ok(conn) => {
            debug!(
                count = conn.products.len(),
                "Retrieved recent products from Shopify"
            );
            products_to_search_results(conn.products)
        }
        Err(e) => {
            warn!(?e, "Failed to fetch products from Shopify");
            vec![]
        }
    };

    let template = ManufacturingNewTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/manufacturing".to_string(),
        recent_products,
    };
    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
}

/// Create batch.
#[instrument(skip(state, form), fields(admin_id = %user.id.as_i32()))]
pub async fn manufacturing_create(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Form(form): Form<CreateBatchForm>,
) -> impl IntoResponse {
    debug!(
        batch_number = %form.batch_number,
        product_id = %form.shopify_product_id,
        quantity = form.quantity,
        "Creating new manufacturing batch"
    );
    let repo = ManufacturingRepository::new(state.pool());

    let input = CreateBatchInput {
        batch_number: form.batch_number,
        shopify_product_id: form.shopify_product_id,
        shopify_variant_id: form.shopify_variant_id,
        quantity: form.quantity,
        manufacture_date: form.manufacture_date,
        raw_cost_per_item: form.raw_cost_per_item,
        label_cost_per_item: form.label_cost_per_item,
        outer_carton_cost_per_item: form.outer_carton_cost_per_item,
        currency_code: form.currency_code.unwrap_or_else(|| "USD".to_string()),
        notes: form.notes,
    };

    match repo.create_batch(&input).await {
        Ok(batch) => {
            info!(batch_id = batch.id.as_i32(), "Created manufacturing batch");
            Redirect::to(&format!("/financials/manufacturing/{}", batch.id.as_i32()))
                .into_response()
        }
        Err(RepositoryError::Conflict(msg)) => {
            warn!(%msg, "Batch creation conflict - duplicate batch number");
            Html(format!("Error: {msg}")).into_response()
        }
        Err(e) => {
            tracing::error!(?e, "Failed to create batch");
            Html(format!("Error: {e}")).into_response()
        }
    }
}

/// Build a `BatchView` from batch data with product info.
fn build_batch_view(
    batch: &crate::models::manufacturing::ManufacturingBatch,
    lots_received: i64,
    product_info: (Option<String>, Option<String>, Option<String>),
) -> BatchView {
    let (product_title, product_image, variant_title) = product_info;
    BatchView {
        id: batch.id.as_i32(),
        batch_number: batch.batch_number.clone(),
        shopify_product_id: batch.shopify_product_id.clone(),
        shopify_variant_id: batch.shopify_variant_id.clone(),
        product_title,
        product_image,
        variant_title,
        product_short_id: extract_short_id(&batch.shopify_product_id),
        quantity: batch.quantity,
        manufacture_date: batch.manufacture_date.to_string(),
        raw_cost_per_item: format!("{:.4}", batch.raw_cost_per_item),
        label_cost_per_item: format!("{:.4}", batch.label_cost_per_item),
        outer_carton_cost_per_item: format!("{:.4}", batch.outer_carton_cost_per_item),
        cost_per_unit: format!("{:.4}", batch.cost_per_unit),
        total_batch_cost: format!("{:.2}", batch.total_batch_cost),
        currency_code: batch.currency_code.clone(),
        notes: batch.notes.clone(),
        lots_received,
    }
}

/// Batch detail page.
#[instrument(skip(state), fields(admin_id = %user.id.as_i32(), batch_id = id))]
pub async fn manufacturing_show(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    debug!("Viewing manufacturing batch details");
    let mfg_repo = ManufacturingRepository::new(state.pool());
    let lot_repo = InventoryLotRepository::new(state.pool());
    let batch_id = naked_pineapple_core::ManufacturingBatchId::new(id);

    let batch = match mfg_repo.get_batch(batch_id).await {
        Ok(Some(batch)) => {
            debug!(batch_number = %batch.batch_number, "Found batch");
            batch
        }
        Ok(None) => {
            warn!("Batch not found, redirecting to index");
            return Redirect::to("/financials/manufacturing").into_response();
        }
        Err(e) => {
            tracing::error!(?e, "Failed to get batch");
            return Html(format!("Error: {e}")).into_response();
        }
    };

    let lots_received = mfg_repo.get_lots_received(batch_id).await.unwrap_or(0);
    let lots = lot_repo
        .list_lots_for_batch(batch_id)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(?e, "Failed to list lots");
            Vec::new()
        });

    let product_info = fetch_product_info(
        state.shopify(),
        &batch.shopify_product_id,
        batch.shopify_variant_id.as_deref(),
    )
    .await;
    let locations = fetch_locations(state.shopify()).await;

    let batch_view = build_batch_view(&batch, lots_received, product_info);
    let lot_views = lots_to_views(&lots, &locations);

    let template = ManufacturingShowTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/manufacturing".to_string(),
        batch: batch_view,
        lots: lot_views,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// Edit batch form.
#[instrument(skip(state), fields(admin_id = %user.id.as_i32(), batch_id = id))]
pub async fn manufacturing_edit(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    debug!("Rendering batch edit form");
    let repo = ManufacturingRepository::new(state.pool());
    let batch_id = naked_pineapple_core::ManufacturingBatchId::new(id);

    let batch = match repo.get_batch(batch_id).await {
        Ok(Some(batch)) => {
            debug!(batch_number = %batch.batch_number, "Found batch for editing");
            batch
        }
        Ok(None) => {
            warn!("Batch not found for editing, redirecting to index");
            return Redirect::to("/financials/manufacturing").into_response();
        }
        Err(e) => {
            tracing::error!(?e, "Failed to get batch");
            return Html(format!("Error: {e}")).into_response();
        }
    };

    let lots_received = repo.get_lots_received(batch_id).await.unwrap_or(0);

    // Fetch product info from Shopify
    let (product_title, product_image, variant_title) =
        if let Ok(Some(product)) = state.shopify().get_product(&batch.shopify_product_id).await {
            let title = Some(product.title.clone());
            let image = product.images.first().map(|img| img.url.clone());
            let var_title = batch.shopify_variant_id.as_ref().and_then(|vid| {
                product
                    .variants
                    .iter()
                    .find(|v| &v.id == vid)
                    .map(|v| v.title.clone())
            });
            (title, image, var_title)
        } else {
            (None, None, None)
        };

    let batch_view = BatchView {
        id: batch.id.as_i32(),
        batch_number: batch.batch_number.clone(),
        shopify_product_id: batch.shopify_product_id.clone(),
        shopify_variant_id: batch.shopify_variant_id.clone(),
        product_title,
        product_image,
        variant_title,
        product_short_id: extract_short_id(&batch.shopify_product_id),
        quantity: batch.quantity,
        manufacture_date: batch.manufacture_date.to_string(),
        raw_cost_per_item: format!("{:.4}", batch.raw_cost_per_item),
        label_cost_per_item: format!("{:.4}", batch.label_cost_per_item),
        outer_carton_cost_per_item: format!("{:.4}", batch.outer_carton_cost_per_item),
        cost_per_unit: format!("{:.4}", batch.cost_per_unit),
        total_batch_cost: format!("{:.2}", batch.total_batch_cost),
        currency_code: batch.currency_code,
        notes: batch.notes,
        lots_received,
    };

    let template = ManufacturingEditTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/manufacturing".to_string(),
        batch: batch_view,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// Update batch.
#[instrument(skip(state, form), fields(admin_id = %user.id.as_i32(), batch_id = id))]
pub async fn manufacturing_update(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path(id): Path<i32>,
    Form(form): Form<UpdateBatchForm>,
) -> impl IntoResponse {
    debug!("Updating manufacturing batch");
    let repo = ManufacturingRepository::new(state.pool());
    let batch_id = naked_pineapple_core::ManufacturingBatchId::new(id);

    let input = UpdateBatchInput {
        batch_number: form.batch_number,
        quantity: form.quantity,
        manufacture_date: form.manufacture_date,
        raw_cost_per_item: form.raw_cost_per_item,
        label_cost_per_item: form.label_cost_per_item,
        outer_carton_cost_per_item: form.outer_carton_cost_per_item,
        currency_code: form.currency_code,
        notes: form.notes,
    };

    match repo.update_batch(batch_id, &input).await {
        Ok(_) => {
            info!("Updated manufacturing batch");
            Redirect::to(&format!("/financials/manufacturing/{id}")).into_response()
        }
        Err(RepositoryError::NotFound) => {
            warn!("Batch not found for update, redirecting to index");
            Redirect::to("/financials/manufacturing").into_response()
        }
        Err(e) => {
            tracing::error!(?e, "Failed to update batch");
            Html(format!("Error: {e}")).into_response()
        }
    }
}

/// Delete batch.
#[instrument(skip(state), fields(admin_id = %user.id.as_i32(), batch_id = id))]
pub async fn manufacturing_delete(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    debug!("Deleting manufacturing batch");
    let repo = ManufacturingRepository::new(state.pool());
    let batch_id = naked_pineapple_core::ManufacturingBatchId::new(id);

    match repo.delete_batch(batch_id).await {
        Ok(_) => {
            info!("Deleted manufacturing batch");
            Redirect::to("/financials/manufacturing")
        }
        Err(e) => {
            tracing::error!(?e, "Failed to delete batch");
            Redirect::to(&format!("/financials/manufacturing/{id}"))
        }
    }
}

// =============================================================================
// Lot Handlers
// =============================================================================

/// New lot form.
#[instrument(skip(state), fields(admin_id = %user.id.as_i32(), batch_id))]
pub async fn lot_new(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path(batch_id): Path<i32>,
) -> impl IntoResponse {
    debug!("Rendering new lot form for batch");
    let repo = ManufacturingRepository::new(state.pool());
    let id = naked_pineapple_core::ManufacturingBatchId::new(batch_id);

    let batch = match repo.get_batch(id).await {
        Ok(Some(batch)) => {
            debug!(batch_number = %batch.batch_number, "Found parent batch for new lot");
            batch
        }
        Ok(None) => {
            warn!("Parent batch not found, redirecting to index");
            return Redirect::to("/financials/manufacturing").into_response();
        }
        Err(e) => {
            tracing::error!(?e, "Failed to get batch");
            return Html(format!("Error: {e}")).into_response();
        }
    };

    let lots_received = repo.get_lots_received(id).await.unwrap_or(0);

    // Fetch product info from Shopify
    let (product_title, product_image, variant_title) =
        if let Ok(Some(product)) = state.shopify().get_product(&batch.shopify_product_id).await {
            let title = Some(product.title.clone());
            let image = product.images.first().map(|img| img.url.clone());
            let var_title = batch.shopify_variant_id.as_ref().and_then(|vid| {
                product
                    .variants
                    .iter()
                    .find(|v| &v.id == vid)
                    .map(|v| v.title.clone())
            });
            (title, image, var_title)
        } else {
            (None, None, None)
        };

    // Fetch locations for dropdown
    let locations = fetch_locations(state.shopify()).await;

    let batch_view = BatchView {
        id: batch.id.as_i32(),
        batch_number: batch.batch_number.clone(),
        shopify_product_id: batch.shopify_product_id.clone(),
        shopify_variant_id: batch.shopify_variant_id.clone(),
        product_title,
        product_image,
        variant_title,
        product_short_id: extract_short_id(&batch.shopify_product_id),
        quantity: batch.quantity,
        manufacture_date: batch.manufacture_date.to_string(),
        raw_cost_per_item: format!("{:.4}", batch.raw_cost_per_item),
        label_cost_per_item: format!("{:.4}", batch.label_cost_per_item),
        outer_carton_cost_per_item: format!("{:.4}", batch.outer_carton_cost_per_item),
        cost_per_unit: format!("{:.4}", batch.cost_per_unit),
        total_batch_cost: format!("{:.2}", batch.total_batch_cost),
        currency_code: batch.currency_code,
        notes: batch.notes,
        lots_received,
    };

    let template = LotNewTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/manufacturing".to_string(),
        batch: batch_view,
        locations,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// Create lot.
#[instrument(skip(state, form), fields(admin_id = %user.id.as_i32(), batch_id))]
pub async fn lot_create(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path(batch_id): Path<i32>,
    Form(form): Form<CreateLotForm>,
) -> impl IntoResponse {
    debug!(
        lot_number = %form.lot_number,
        quantity = form.quantity,
        "Creating new inventory lot"
    );
    let repo = InventoryLotRepository::new(state.pool());
    let id = naked_pineapple_core::ManufacturingBatchId::new(batch_id);

    let input = CreateLotInput {
        batch_id: id,
        lot_number: form.lot_number,
        quantity: form.quantity,
        received_date: form.received_date,
        shopify_location_id: form.shopify_location_id,
        notes: form.notes,
    };

    match repo.create_lot(&input).await {
        Ok(lot) => {
            info!(lot_id = lot.id.as_i32(), "Created inventory lot");
            Redirect::to(&format!("/financials/manufacturing/{batch_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(?e, "Failed to create lot");
            Html(format!("Error: {e}")).into_response()
        }
    }
}

/// Edit lot form.
#[instrument(skip(state), fields(admin_id = %user.id.as_i32(), batch_id, lot_id))]
pub async fn lot_edit(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path((batch_id, lot_id)): Path<(i32, i32)>,
) -> impl IntoResponse {
    debug!("Rendering lot edit form");
    let mfg_repo = ManufacturingRepository::new(state.pool());
    let lot_repo = InventoryLotRepository::new(state.pool());
    let b_id = naked_pineapple_core::ManufacturingBatchId::new(batch_id);
    let l_id = naked_pineapple_core::InventoryLotId::new(lot_id);

    let batch = match mfg_repo.get_batch(b_id).await {
        Ok(Some(batch)) => {
            debug!(batch_number = %batch.batch_number, "Found parent batch");
            batch
        }
        Ok(None) => {
            warn!("Parent batch not found for lot edit, redirecting to index");
            return Redirect::to("/financials/manufacturing").into_response();
        }
        Err(e) => {
            tracing::error!(?e, "Failed to get batch");
            return Html(format!("Error: {e}")).into_response();
        }
    };

    let lot_with_remaining = match lot_repo.get_lot_with_remaining(l_id).await {
        Ok(Some(lot)) => {
            debug!(lot_number = %lot.lot.lot_number, "Found lot for editing");
            lot
        }
        Ok(None) => {
            warn!("Lot not found for editing, redirecting to batch");
            return Redirect::to(&format!("/financials/manufacturing/{batch_id}")).into_response();
        }
        Err(e) => {
            tracing::error!(?e, "Failed to get lot");
            return Html(format!("Error: {e}")).into_response();
        }
    };

    let lots_received = mfg_repo.get_lots_received(b_id).await.unwrap_or(0);
    let product_info = fetch_product_info(
        state.shopify(),
        &batch.shopify_product_id,
        batch.shopify_variant_id.as_deref(),
    )
    .await;
    let locations = fetch_locations(state.shopify()).await;

    let batch_view = build_batch_view(&batch, lots_received, product_info);
    let lot_view = build_lot_view(&lot_with_remaining, &locations);

    let template = LotEditTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/manufacturing".to_string(),
        batch: batch_view,
        lot: lot_view,
        locations,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// Update lot.
#[instrument(skip(state, form), fields(admin_id = %user.id.as_i32(), batch_id, lot_id))]
pub async fn lot_update(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path((batch_id, lot_id)): Path<(i32, i32)>,
    Form(form): Form<UpdateLotForm>,
) -> impl IntoResponse {
    debug!("Updating inventory lot");
    let repo = InventoryLotRepository::new(state.pool());
    let l_id = naked_pineapple_core::InventoryLotId::new(lot_id);

    let input = UpdateLotInput {
        lot_number: form.lot_number,
        quantity: form.quantity,
        received_date: form.received_date,
        shopify_location_id: form.shopify_location_id,
        notes: form.notes,
    };

    match repo.update_lot(l_id, &input).await {
        Ok(_) => {
            info!("Updated inventory lot");
            Redirect::to(&format!("/financials/manufacturing/{batch_id}")).into_response()
        }
        Err(RepositoryError::NotFound) => {
            warn!("Lot not found for update, redirecting to batch");
            Redirect::to(&format!("/financials/manufacturing/{batch_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(?e, "Failed to update lot");
            Html(format!("Error: {e}")).into_response()
        }
    }
}

/// Delete lot.
#[instrument(skip(state), fields(admin_id = %user.id.as_i32(), batch_id, lot_id))]
pub async fn lot_delete(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path((batch_id, lot_id)): Path<(i32, i32)>,
) -> impl IntoResponse {
    debug!("Deleting inventory lot");
    let repo = InventoryLotRepository::new(state.pool());
    let l_id = naked_pineapple_core::InventoryLotId::new(lot_id);

    match repo.delete_lot(l_id).await {
        Ok(_) => {
            info!("Deleted inventory lot");
            Redirect::to(&format!("/financials/manufacturing/{batch_id}"))
        }
        Err(e) => {
            tracing::error!(?e, "Failed to delete lot");
            Redirect::to(&format!("/financials/manufacturing/{batch_id}"))
        }
    }
}

// =============================================================================
// Router
// =============================================================================

/// Build the financials router.
pub fn router() -> Router<AppState> {
    Router::new()
        // Financials landing
        .route("/financials", get(index))
        // Profit margins
        .route("/financials/margins", get(margins))
        // Manufacturing batches
        .route(
            "/financials/manufacturing",
            get(manufacturing_index).post(manufacturing_create),
        )
        .route("/financials/manufacturing/new", get(manufacturing_new))
        .route(
            "/financials/manufacturing/{id}",
            get(manufacturing_show).post(manufacturing_update),
        )
        .route(
            "/financials/manufacturing/{id}/edit",
            get(manufacturing_edit),
        )
        .route(
            "/financials/manufacturing/{id}/delete",
            post(manufacturing_delete),
        )
        // Inventory lots (nested under batches)
        .route(
            "/financials/manufacturing/{batch_id}/lots/new",
            get(lot_new),
        )
        .route(
            "/financials/manufacturing/{batch_id}/lots",
            post(lot_create),
        )
        .route(
            "/financials/manufacturing/{batch_id}/lots/{id}/edit",
            get(lot_edit),
        )
        .route(
            "/financials/manufacturing/{batch_id}/lots/{id}",
            post(lot_update),
        )
        .route(
            "/financials/manufacturing/{batch_id}/lots/{id}/delete",
            post(lot_delete),
        )
}

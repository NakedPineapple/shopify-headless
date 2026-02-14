//! Amazon analytics route handlers.
//!
//! Provides Amazon sales performance dashboard (from cached order data)
//! and competitive pricing intelligence (from the Pricing API).

use askama::Template;
use axum::{
    extract::{Query, State},
    response::Html,
};
use tracing::{debug, instrument, warn};

use crate::{
    db::{AmazonOrderRepository, AmazonProductMappingRepository},
    filters,
    middleware::auth::RequireAdminAuth,
    state::AppState,
};

use super::analytics::AnalyticsQuery;
use super::dashboard::AdminUserView;

// =============================================================================
// View Types
// =============================================================================

/// Summary view for the Amazon sales dashboard.
struct AmazonSalesSummaryView {
    total_revenue: String,
    order_count: String,
    aov: String,
    shipped: String,
    pending: String,
    canceled: String,
    fba_count: String,
    merchant_count: String,
    prime_count: String,
    non_prime_count: String,
}

/// Pricing view for a single ASIN.
struct AsinPricingView {
    asin: String,
    amazon_sku: String,
    shopify_product_id: String,
    our_price: Option<String>,
    buy_box_price: Option<String>,
    buy_box_is_ours: bool,
    num_offers: i32,
    sales_rank: Option<i32>,
    status: String,
    status_color: String,
}

// =============================================================================
// Templates
// =============================================================================

/// Amazon sales dashboard template.
#[derive(Template)]
#[template(path = "analytics/amazon.html")]
struct AmazonSalesTemplate {
    admin_user: AdminUserView,
    current_path: String,
    summary: AmazonSalesSummaryView,
    current_range: String,
    custom_start: String,
    custom_end: String,
    trend_labels: String,
    trend_data: String,
    fba_count: i64,
    merchant_count: i64,
    has_data: bool,
}

/// Amazon pricing analysis template.
#[derive(Template)]
#[template(path = "analytics/amazon_pricing.html")]
struct AmazonPricingTemplate {
    admin_user: AdminUserView,
    current_path: String,
    items: Vec<AsinPricingView>,
    mapped_count: usize,
    winning_count: usize,
    behind_count: usize,
    connected: bool,
    has_mappings: bool,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// Amazon sales dashboard page.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn amazon_sales(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AnalyticsQuery>,
) -> Html<String> {
    debug!("Fetching Amazon sales dashboard");
    let (start, end) = query_to_dates(&query);
    let repo = AmazonOrderRepository::new(state.pool());

    let (rev, daily, fulfillment, prime, statuses) = tokio::join!(
        repo.revenue_summary(start, end),
        repo.daily_revenue(start, end),
        repo.fulfillment_breakdown(start, end),
        repo.prime_count(start, end),
        repo.status_breakdown(start, end),
    );

    let summary = build_sales_summary(&rev, &statuses, &fulfillment, &prime);
    let (trend_labels, trend_data) = build_daily_trend_json(&daily);
    let (fba_count, merchant_count) = build_fulfillment_counts(&fulfillment);
    let has_data = rev.as_ref().is_ok_and(|r| r.order_count > 0);

    let template = AmazonSalesTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/analytics/amazon".to_string(),
        summary,
        current_range: query.current_range().to_string(),
        custom_start: query.start.clone().unwrap_or_default(),
        custom_end: query.end.clone().unwrap_or_default(),
        trend_labels,
        trend_data,
        fba_count,
        merchant_count,
        has_data,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {e}");
        "Internal Server Error".to_string()
    }))
}

/// Amazon pricing analysis page.
///
/// Fetches competitive pricing from the SP-API (cached 5 min) and joins
/// with product mappings to show buy box position.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn amazon_pricing(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
) -> Html<String> {
    debug!("Fetching Amazon pricing analysis");

    let Some(client) = state.amazon() else {
        return render_pricing_template(&admin, vec![], 0, false, true);
    };

    let mapping_repo = AmazonProductMappingRepository::new(state.pool());
    let mappings = mapping_repo.list().await.unwrap_or_default();
    if mappings.is_empty() {
        return render_pricing_template(&admin, vec![], 0, true, false);
    }

    let mut asins: Vec<String> = mappings.iter().map(|m| m.asin.clone()).collect();
    asins.sort();
    asins.dedup();
    let cache_key = asins.join(",");

    let pricing_results = if let Some(cached) = state.pricing_cache().get(&cache_key).await {
        cached
    } else {
        match client.get_competitive_pricing(&asins).await {
            Ok(results) => {
                state
                    .pricing_cache()
                    .insert(cache_key, results.clone())
                    .await;
                results
            }
            Err(e) => {
                warn!("Failed to fetch competitive pricing: {e}");
                vec![]
            }
        }
    };

    let items = build_pricing_views(&mappings, &pricing_results);
    let winning_count = items.iter().filter(|i| i.status == "Winning").count();

    render_pricing_template(&admin, items, winning_count, true, true)
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Convert analytics query to `NaiveDate` pair.
fn query_to_dates(query: &AnalyticsQuery) -> (chrono::NaiveDate, chrono::NaiveDate) {
    let now = chrono::Utc::now().date_naive();
    if let (Some(s), Some(e)) = (&query.start, &query.end) {
        let start = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .unwrap_or(now - chrono::Duration::days(30));
        let end = chrono::NaiveDate::parse_from_str(e, "%Y-%m-%d").unwrap_or(now);
        (start, end)
    } else {
        let days = match query.range.as_deref() {
            Some("7d") => 7,
            Some("90d") => 90,
            Some("ytd") => 365,
            _ => 30,
        };
        (now - chrono::Duration::days(days), now)
    }
}

/// Build summary view from query results.
fn build_sales_summary(
    rev: &Result<crate::db::AmazonRevenueSummary, crate::db::RepositoryError>,
    statuses: &Result<Vec<crate::db::StatusBreakdown>, crate::db::RepositoryError>,
    fulfillment: &Result<Vec<crate::db::FulfillmentBreakdown>, crate::db::RepositoryError>,
    prime: &Result<(i64, i64), crate::db::RepositoryError>,
) -> AmazonSalesSummaryView {
    let (revenue, count, aov) = rev.as_ref().map_or((0.0, 0, 0.0), |r| {
        (r.total_revenue, r.order_count, r.average_order_value)
    });

    let status_list = statuses.as_ref().unwrap_or(&Vec::new()).clone();
    let shipped = status_count(&status_list, "Shipped");
    let pending = status_count(&status_list, "Pending") + status_count(&status_list, "Unshipped");
    let canceled = status_count(&status_list, "Canceled");

    let ff_list = fulfillment.as_ref().unwrap_or(&Vec::new()).clone();
    let fba = ff_count(&ff_list, "AFN");
    let merchant = ff_count(&ff_list, "MFN");

    let (prime_c, non_prime_c) = prime.as_ref().copied().unwrap_or((0, 0));

    AmazonSalesSummaryView {
        total_revenue: format_currency(revenue),
        order_count: count.to_string(),
        aov: format_currency(aov),
        shipped: shipped.to_string(),
        pending: pending.to_string(),
        canceled: canceled.to_string(),
        fba_count: fba.to_string(),
        merchant_count: merchant.to_string(),
        prime_count: prime_c.to_string(),
        non_prime_count: non_prime_c.to_string(),
    }
}

/// Get count for a specific status from the breakdown.
fn status_count(statuses: &[crate::db::StatusBreakdown], name: &str) -> i64 {
    statuses
        .iter()
        .find(|s| s.status == name)
        .map_or(0, |s| s.count)
}

/// Get count for a specific fulfillment channel from the breakdown.
fn ff_count(ff: &[crate::db::FulfillmentBreakdown], name: &str) -> i64 {
    ff.iter().find(|f| f.channel == name).map_or(0, |f| f.count)
}

/// Build Chart.js JSON arrays for the daily trend chart.
fn build_daily_trend_json(
    daily: &Result<Vec<crate::db::AmazonDailyRevenue>, crate::db::RepositoryError>,
) -> (String, String) {
    let Ok(days) = daily else {
        return ("[]".to_string(), "[]".to_string());
    };

    let labels: Vec<String> = days
        .iter()
        .map(|d| d.date.format("%b %d").to_string())
        .collect();
    let data: Vec<String> = days.iter().map(|d| format!("{:.2}", d.revenue)).collect();

    let labels_json = serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_string());
    let data_json = format!("[{}]", data.join(","));
    (labels_json, data_json)
}

/// Extract FBA vs Merchant counts from fulfillment breakdown.
fn build_fulfillment_counts(
    fulfillment: &Result<Vec<crate::db::FulfillmentBreakdown>, crate::db::RepositoryError>,
) -> (i64, i64) {
    let ff_list = fulfillment.as_ref().unwrap_or(&Vec::new()).clone();
    (ff_count(&ff_list, "AFN"), ff_count(&ff_list, "MFN"))
}

/// Build pricing view items by joining mappings with pricing results.
fn build_pricing_views(
    mappings: &[crate::db::AmazonProductMapping],
    pricing: &[naked_pineapple_services::amazon_sp::PricingResult],
) -> Vec<AsinPricingView> {
    mappings
        .iter()
        .map(|m| {
            let pr = pricing.iter().find(|p| p.asin.as_deref() == Some(&m.asin));
            build_single_pricing_view(m, pr)
        })
        .collect()
}

/// Build a pricing view for a single mapping + pricing result.
fn build_single_pricing_view(
    mapping: &crate::db::AmazonProductMapping,
    pr: Option<&naked_pineapple_services::amazon_sp::PricingResult>,
) -> AsinPricingView {
    let (buy_box_price, buy_box_is_ours, num_offers, sales_rank) =
        pr.map_or((None, false, 0, None), |p| {
            (
                p.buy_box_price.map(|v| format!("${v:.2}")),
                p.belongs_to_requester.unwrap_or(false),
                p.num_offers,
                p.sales_rank,
            )
        });

    let (status, status_color) = determine_pricing_status(buy_box_is_ours, buy_box_price.as_ref());

    AsinPricingView {
        asin: mapping.asin.clone(),
        amazon_sku: mapping.amazon_sku.clone(),
        shopify_product_id: mapping.shopify_product_id.clone(),
        our_price: None,
        buy_box_price,
        buy_box_is_ours,
        num_offers,
        sales_rank,
        status,
        status_color,
    }
}

/// Determine competitive status and badge color.
fn determine_pricing_status(
    buy_box_is_ours: bool,
    buy_box_price: Option<&String>,
) -> (String, String) {
    if buy_box_is_ours {
        ("Winning".to_string(), "green".to_string())
    } else if buy_box_price.is_some() {
        ("Behind".to_string(), "red".to_string())
    } else {
        ("Unknown".to_string(), "gray".to_string())
    }
}

/// Render the pricing template with the given data.
fn render_pricing_template(
    admin: &crate::models::session::CurrentAdmin,
    items: Vec<AsinPricingView>,
    winning_count: usize,
    has_mappings: bool,
    connected: bool,
) -> Html<String> {
    let behind_count = items.iter().filter(|i| i.status == "Behind").count();
    let mapped_count = items.len();

    let template = AmazonPricingTemplate {
        admin_user: AdminUserView::from(admin),
        current_path: "/analytics/amazon/pricing".to_string(),
        items,
        mapped_count,
        winning_count,
        behind_count,
        connected,
        has_mappings,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {e}");
        "Internal Server Error".to_string()
    }))
}

/// Format a number as currency.
fn format_currency(amount: f64) -> String {
    if amount >= 1_000_000.0 {
        format!("${:.2}M", amount / 1_000_000.0)
    } else if amount >= 1_000.0 {
        format!("${:.2}K", amount / 1_000.0)
    } else {
        format!("${amount:.2}")
    }
}

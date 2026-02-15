//! Faire analytics route handlers.
//!
//! Provides a wholesale sales performance dashboard for Faire orders
//! including revenue summary, daily trends, retailer breakdown,
//! and first-order metrics.

use askama::Template;
use axum::{
    extract::{Query, State},
    response::Html,
};
use tracing::{debug, instrument};

use crate::{
    db::FaireOrderRepository, filters, middleware::auth::RequireAdminAuth, state::AppState,
};

use super::analytics::AnalyticsQuery;
use super::dashboard::AdminUserView;

// =============================================================================
// View Types
// =============================================================================

/// Summary metrics for the Faire sales dashboard.
struct FaireSummaryView {
    total_revenue: String,
    order_count: String,
    aov: String,
    total_commission: String,
    net_payout: String,
    first_order_count: String,
}

/// Top retailer breakdown for the retailers table.
struct RetailerBreakdownView {
    retailer_name: String,
    order_count: i64,
    revenue: String,
}

// =============================================================================
// Templates
// =============================================================================

/// Faire wholesale sales dashboard template.
#[derive(Template)]
#[template(path = "analytics/faire.html")]
struct FaireAnalyticsTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
    has_data: bool,
    current_range: String,
    custom_start: String,
    custom_end: String,
    summary: FaireSummaryView,
    trend_labels: String,
    trend_data: String,
    unique_retailer_count: i64,
    retailers: Vec<RetailerBreakdownView>,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// Faire wholesale sales dashboard page.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn faire_analytics(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AnalyticsQuery>,
) -> Html<String> {
    debug!("Fetching Faire analytics dashboard");
    let (start, end) = query_to_dates(&query);

    let connected = crate::db::FaireCredentialsRepository::new(state.pool())
        .get_default()
        .await
        .ok()
        .flatten()
        .is_some();

    let order_repo = FaireOrderRepository::new(state.pool());

    let (rev, daily, retailers, first_orders, unique_retailers) = tokio::join!(
        order_repo.revenue_summary(start, end),
        order_repo.daily_revenue(start, end),
        order_repo.retailer_breakdown(start, end),
        order_repo.count_first_orders(),
        order_repo.count_unique_retailers(),
    );

    let (total_orders, total_rev, aov, commission, net) = build_summary(&rev);
    let (trend_labels, trend_data) = build_daily_trend_json(&daily);
    let retailer_views = build_retailer_views(&retailers);
    let first_order_count = first_orders.unwrap_or(0);
    let unique_retailer_count = unique_retailers.unwrap_or(0);
    let has_data = rev.as_ref().is_ok_and(|r| r.total_orders > 0);

    let summary = FaireSummaryView {
        total_revenue: total_rev,
        order_count: total_orders.to_string(),
        aov,
        total_commission: commission,
        net_payout: net,
        first_order_count: first_order_count.to_string(),
    };

    let template = FaireAnalyticsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/analytics/faire".to_string(),
        connected,
        has_data,
        current_range: query.current_range().to_string(),
        custom_start: query.start.clone().unwrap_or_default(),
        custom_end: query.end.clone().unwrap_or_default(),
        summary,
        trend_labels,
        trend_data,
        unique_retailer_count,
        retailers: retailer_views,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {e}");
        "Internal Server Error".to_string()
    }))
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

/// Build summary values from query results.
///
/// Since `FaireRevenueSummary` fields are `String`s (not `f64`), parse them
/// before formatting as currency.
fn build_summary(
    rev: &Result<crate::db::FaireRevenueSummary, crate::db::RepositoryError>,
) -> (i64, String, String, String, String) {
    rev.as_ref().map_or_else(
        |_| {
            (
                0,
                "$0.00".to_string(),
                "$0.00".to_string(),
                "$0.00".to_string(),
                "$0.00".to_string(),
            )
        },
        |r| {
            (
                r.total_orders,
                format_dollar_string(&r.total_revenue),
                format_dollar_string(&r.average_order_value),
                format_dollar_string(&r.total_commission),
                format_dollar_string(&r.net_payout),
            )
        },
    )
}

/// Parse a dollar-amount string and format it as currency.
fn format_dollar_string(amount_str: &str) -> String {
    let amount: f64 = amount_str.parse().unwrap_or(0.0);
    format_currency(amount)
}

/// Build Chart.js JSON arrays for the daily trend chart.
///
/// `FaireDailyRevenue.date` is a `String` and `total_revenue` is a `String`.
fn build_daily_trend_json(
    daily: &Result<Vec<crate::db::FaireDailyRevenue>, crate::db::RepositoryError>,
) -> (String, String) {
    let Ok(days) = daily else {
        return ("[]".to_string(), "[]".to_string());
    };

    let labels: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let data: Vec<String> = days
        .iter()
        .map(|d| {
            let val: f64 = d.total_revenue.parse().unwrap_or(0.0);
            format!("{val:.2}")
        })
        .collect();

    let labels_json = serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_string());
    let data_json = format!("[{}]", data.join(","));
    (labels_json, data_json)
}

/// Build retailer breakdown view models.
fn build_retailer_views(
    retailers: &Result<Vec<crate::db::RetailerBreakdown>, crate::db::RepositoryError>,
) -> Vec<RetailerBreakdownView> {
    let Ok(items) = retailers else {
        return vec![];
    };

    items
        .iter()
        .map(|r| RetailerBreakdownView {
            retailer_name: r.retailer_name.clone(),
            order_count: r.order_count,
            revenue: format_dollar_string(&r.total_revenue),
        })
        .collect()
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

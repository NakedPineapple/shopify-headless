//! TikTok Shop analytics route handlers.
//!
//! Provides a sales performance dashboard for TikTok Shop orders
//! including source breakdowns, creator performance, and affiliate metrics.

use askama::Template;
use axum::{
    extract::{Query, State},
    response::Html,
};
use tracing::{debug, instrument};

use crate::{
    db::TikTokOrderRepository, filters, middleware::auth::RequireAdminAuth, state::AppState,
};

use super::analytics::AnalyticsQuery;
use super::dashboard::AdminUserView;

// =============================================================================
// View Types
// =============================================================================

/// Summary view for the TikTok sales dashboard.
struct TikTokSalesSummaryView {
    total_revenue: String,
    order_count: String,
    aov: String,
    total_commission: String,
    total_platform_fees: String,
}

/// Source breakdown for chart data.
struct SourceBreakdownView {
    source_type: String,
    revenue: f64,
    count: i64,
}

/// Creator breakdown for the top creators table.
struct CreatorBreakdownView {
    creator_username: String,
    order_count: i64,
    revenue: String,
    commission: String,
}

/// Affiliate summary for the affiliate section.
struct AffiliateSummaryView {
    total_orders: i64,
    total_commission: String,
    conversion_rate: String,
}

// =============================================================================
// Templates
// =============================================================================

/// TikTok sales dashboard template.
#[derive(Template)]
#[template(path = "analytics/tiktok.html")]
struct TikTokSalesTemplate {
    admin_user: AdminUserView,
    current_path: String,
    summary: TikTokSalesSummaryView,
    current_range: String,
    custom_start: String,
    custom_end: String,
    trend_labels: String,
    trend_data: String,
    source_labels: String,
    source_data: String,
    source_colors: String,
    creators: Vec<CreatorBreakdownView>,
    affiliate: AffiliateSummaryView,
    has_data: bool,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// TikTok sales dashboard page.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn tiktok_sales(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AnalyticsQuery>,
) -> Html<String> {
    debug!("Fetching TikTok Shop sales dashboard");
    let (start, end) = query_to_dates(&query);
    let repo = TikTokOrderRepository::new(state.pool());

    let (rev, daily, sources, creators, affiliate) = tokio::join!(
        repo.revenue_summary(start, end),
        repo.daily_revenue(start, end),
        repo.source_breakdown(start, end),
        repo.creator_breakdown(start, end, 10),
        repo.affiliate_summary(start, end),
    );

    let summary = build_sales_summary(&rev);
    let (trend_labels, trend_data) = build_daily_trend_json(&daily);
    let (source_labels, source_data, source_colors) = build_source_breakdown_json(&sources);
    let creators = build_creator_views(&creators);
    let affiliate = build_affiliate_view(&affiliate);
    let has_data = rev.as_ref().is_ok_and(|r| r.order_count > 0);

    let template = TikTokSalesTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/analytics/tiktok".to_string(),
        summary,
        current_range: query.current_range().to_string(),
        custom_start: query.start.clone().unwrap_or_default(),
        custom_end: query.end.clone().unwrap_or_default(),
        trend_labels,
        trend_data,
        source_labels,
        source_data,
        source_colors,
        creators,
        affiliate,
        has_data,
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

/// Build summary view from query results.
fn build_sales_summary(
    rev: &Result<crate::db::TikTokRevenueSummary, crate::db::RepositoryError>,
) -> TikTokSalesSummaryView {
    let (revenue, count, aov, commission, fees) =
        rev.as_ref().map_or((0.0, 0, 0.0, 0.0, 0.0), |r| {
            (
                r.total_revenue,
                r.order_count,
                r.average_order_value,
                r.total_commission,
                r.total_platform_fees,
            )
        });

    TikTokSalesSummaryView {
        total_revenue: format_currency(revenue),
        order_count: count.to_string(),
        aov: format_currency(aov),
        total_commission: format_currency(commission),
        total_platform_fees: format_currency(fees),
    }
}

/// Build Chart.js JSON arrays for the daily trend chart.
fn build_daily_trend_json(
    daily: &Result<Vec<crate::db::TikTokDailyRevenue>, crate::db::RepositoryError>,
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

/// Build Chart.js JSON arrays for the source breakdown chart.
fn build_source_breakdown_json(
    sources: &Result<Vec<crate::db::SourceBreakdown>, crate::db::RepositoryError>,
) -> (String, String, String) {
    let Ok(items) = sources else {
        return ("[]".to_string(), "[]".to_string(), "[]".to_string());
    };

    // Chart palette for source types.
    let palette = [
        "#d63a2f", "#3b82f6", "#10b981", "#f59e0b", "#8b5cf6", "#ec4899",
    ];

    let labels: Vec<String> = items.iter().map(|s| s.source_type.clone()).collect();
    let data: Vec<String> = items.iter().map(|s| format!("{:.2}", s.revenue)).collect();
    let colors: Vec<&str> = items
        .iter()
        .enumerate()
        .filter_map(|(i, _)| palette.get(i % palette.len()).copied())
        .collect();

    let labels_json = serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_string());
    let data_json = format!("[{}]", data.join(","));
    let colors_json = serde_json::to_string(&colors).unwrap_or_else(|_| "[]".to_string());
    (labels_json, data_json, colors_json)
}

/// Build creator breakdown view models.
fn build_creator_views(
    creators: &Result<Vec<crate::db::CreatorBreakdown>, crate::db::RepositoryError>,
) -> Vec<CreatorBreakdownView> {
    let Ok(items) = creators else {
        return vec![];
    };

    items
        .iter()
        .map(|c| CreatorBreakdownView {
            creator_username: c.creator_username.clone(),
            order_count: c.order_count,
            revenue: format_currency(c.revenue),
            commission: format_currency(c.commission),
        })
        .collect()
}

/// Build affiliate summary view model.
fn build_affiliate_view(
    affiliate: &Result<crate::db::AffiliateSummary, crate::db::RepositoryError>,
) -> AffiliateSummaryView {
    let (orders, commission, rate) = affiliate.as_ref().map_or((0, 0.0, 0.0), |a| {
        (a.total_orders, a.total_commission, a.conversion_rate)
    });

    AffiliateSummaryView {
        total_orders: orders,
        total_commission: format_currency(commission),
        conversion_rate: format!("{:.1}%", rate * 100.0),
    }
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

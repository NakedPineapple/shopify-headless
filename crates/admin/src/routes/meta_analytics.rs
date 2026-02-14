//! Meta Commerce analytics route handlers.
//!
//! Provides a sales performance dashboard for Facebook Shop and Instagram
//! Shopping orders (from cached order data).

use askama::Template;
use axum::{
    extract::{Query, State},
    response::Html,
};
use tracing::{debug, instrument};

use crate::{
    db::MetaOrderRepository, filters, middleware::auth::RequireAdminAuth, state::AppState,
};

use super::analytics::AnalyticsQuery;
use super::dashboard::AdminUserView;

// =============================================================================
// View Types
// =============================================================================

/// Summary view for the Meta sales dashboard.
struct MetaSalesSummaryView {
    total_revenue: String,
    order_count: String,
    aov: String,
    facebook_count: String,
    facebook_revenue: String,
    instagram_count: String,
    instagram_revenue: String,
}

// =============================================================================
// Templates
// =============================================================================

/// Meta sales dashboard template.
#[derive(Template)]
#[template(path = "analytics/meta.html")]
struct MetaSalesTemplate {
    admin_user: AdminUserView,
    current_path: String,
    summary: MetaSalesSummaryView,
    current_range: String,
    custom_start: String,
    custom_end: String,
    trend_labels: String,
    trend_data: String,
    facebook_count: i64,
    instagram_count: i64,
    has_data: bool,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// Meta sales dashboard page.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn meta_sales(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AnalyticsQuery>,
) -> Html<String> {
    debug!("Fetching Meta Commerce sales dashboard");
    let (start, end) = query_to_dates(&query);
    let repo = MetaOrderRepository::new(state.pool());

    let (rev, daily, channels, statuses) = tokio::join!(
        repo.revenue_summary(start, end),
        repo.daily_revenue(start, end),
        repo.channel_breakdown(start, end),
        repo.status_breakdown(start, end),
    );

    // Suppress unused variable warnings for statuses — reserved for future use
    let _ = &statuses;

    let summary = build_sales_summary(&rev, &channels);
    let (trend_labels, trend_data) = build_daily_trend_json(&daily);
    let (facebook_count, instagram_count) = build_channel_counts(&channels);
    let has_data = rev.as_ref().is_ok_and(|r| r.order_count > 0);

    let template = MetaSalesTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/analytics/meta".to_string(),
        summary,
        current_range: query.current_range().to_string(),
        custom_start: query.start.clone().unwrap_or_default(),
        custom_end: query.end.clone().unwrap_or_default(),
        trend_labels,
        trend_data,
        facebook_count,
        instagram_count,
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
    rev: &Result<crate::db::MetaRevenueSummary, crate::db::RepositoryError>,
    channels: &Result<Vec<crate::db::ChannelBreakdown>, crate::db::RepositoryError>,
) -> MetaSalesSummaryView {
    let (revenue, count, aov) = rev.as_ref().map_or((0.0, 0, 0.0), |r| {
        (r.total_revenue, r.order_count, r.average_order_value)
    });

    let channel_list = channels.as_ref().unwrap_or(&Vec::new()).clone();

    let fb = channel_list.iter().find(|c| c.channel == "facebook");
    let ig = channel_list.iter().find(|c| c.channel == "instagram");

    MetaSalesSummaryView {
        total_revenue: format_currency(revenue),
        order_count: count.to_string(),
        aov: format_currency(aov),
        facebook_count: fb.map_or(0, |c| c.count).to_string(),
        facebook_revenue: format_currency(fb.map_or(0.0, |c| c.revenue)),
        instagram_count: ig.map_or(0, |c| c.count).to_string(),
        instagram_revenue: format_currency(ig.map_or(0.0, |c| c.revenue)),
    }
}

/// Build Chart.js JSON arrays for the daily trend chart.
fn build_daily_trend_json(
    daily: &Result<Vec<crate::db::MetaDailyRevenue>, crate::db::RepositoryError>,
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

/// Extract Facebook vs Instagram counts from channel breakdown.
fn build_channel_counts(
    channels: &Result<Vec<crate::db::ChannelBreakdown>, crate::db::RepositoryError>,
) -> (i64, i64) {
    let channel_list = channels.as_ref().unwrap_or(&Vec::new()).clone();
    let fb = channel_list
        .iter()
        .find(|c| c.channel == "facebook")
        .map_or(0, |c| c.count);
    let ig = channel_list
        .iter()
        .find(|c| c.channel == "instagram")
        .map_or(0, |c| c.count);
    (fb, ig)
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

//! Analytics route handlers.
//!
//! Provides channel analytics and sales performance data from Shopify.

#![allow(clippy::used_underscore_binding)]

use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::Html,
};
use serde::Deserialize;
use tracing::{debug, info, instrument, warn};

use crate::{
    db::ExpenseRepository,
    filters,
    middleware::auth::RequireAdminAuth,
    models::expense::ChannelAdSpend,
    shopify::types::{
        AnalyticsSummary, ChannelDailyMetrics, ChannelMetrics, DailyMetrics, DateRange,
        SalesChannel,
    },
    state::AppState,
};

use super::dashboard::AdminUserView;

// =============================================================================
// Query Parameters
// =============================================================================

/// Query parameters for analytics pages.
#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
    /// Date range preset: "7d", "30d", "90d", "ytd", or custom
    pub range: Option<String>,
    /// Custom start date (YYYY-MM-DD)
    pub start: Option<String>,
    /// Custom end date (YYYY-MM-DD)
    pub end: Option<String>,
}

impl AnalyticsQuery {
    /// Convert query params to a `DateRange`.
    fn to_date_range(&self) -> DateRange {
        // Check for custom dates first
        if let (Some(start), Some(end)) = (&self.start, &self.end) {
            return DateRange::new(start.clone(), end.clone());
        }

        // Use preset
        match self.range.as_deref() {
            Some("7d") => DateRange::last_days(7),
            Some("90d") => DateRange::last_days(90),
            Some("ytd") => DateRange::new("-1y", "today"),
            _ => DateRange::last_days(30), // Default to 30 days
        }
    }

    /// Get the current range selection for UI highlighting.
    #[must_use]
    pub fn current_range(&self) -> &str {
        self.range.as_deref().unwrap_or("30d")
    }
}

// =============================================================================
// View Types
// =============================================================================

/// Channel metrics view for templates.
#[derive(Debug, Clone)]
pub struct ChannelMetricsView {
    pub channel_name: String,
    pub total_sales: String,
    pub net_sales: String,
    pub orders: String,
    pub units_sold: String,
    pub average_order_value: String,
    pub percentage_of_total: String,
}

impl ChannelMetricsView {
    fn from_metrics(metrics: &ChannelMetrics, total_sales: f64) -> Self {
        let percentage = if total_sales > 0.0 {
            (metrics.total_sales / total_sales) * 100.0
        } else {
            0.0
        };

        Self {
            channel_name: metrics.channel_name.clone(),
            total_sales: format_currency(metrics.total_sales),
            net_sales: format_currency(metrics.net_sales),
            orders: metrics.orders.to_string(),
            units_sold: metrics.units_sold.to_string(),
            average_order_value: format_currency(metrics.average_order_value),
            percentage_of_total: format!("{percentage:.1}%"),
        }
    }
}

/// Sales channel view for templates.
#[derive(Debug, Clone)]
pub struct SalesChannelView {
    pub id: String,
    pub name: String,
    pub app_title: Option<String>,
    pub auto_publish: bool,
}

impl From<&SalesChannel> for SalesChannelView {
    fn from(channel: &SalesChannel) -> Self {
        Self {
            id: channel.id.clone(),
            name: channel.name.clone(),
            app_title: channel.app.as_ref().map(|a| a.title.clone()),
            auto_publish: channel.auto_publish,
        }
    }
}

/// Daily metrics view for trend charts.
#[derive(Debug, Clone)]
pub struct DailyMetricsView {
    pub date: String,
    pub total_sales: String,
    pub total_sales_raw: f64,
    pub orders: i64,
}

impl From<&DailyMetrics> for DailyMetricsView {
    fn from(m: &DailyMetrics) -> Self {
        Self {
            date: m.date.clone(),
            total_sales: format_currency(m.total_sales),
            total_sales_raw: m.total_sales,
            orders: m.orders,
        }
    }
}

/// Summary metrics view for templates.
#[derive(Debug, Clone)]
pub struct AnalyticsSummaryView {
    pub total_sales: String,
    pub total_net_sales: String,
    pub total_orders: String,
    pub total_units: String,
    pub average_order_value: String,
    pub channels: Vec<ChannelMetricsView>,
}

impl From<&AnalyticsSummary> for AnalyticsSummaryView {
    fn from(summary: &AnalyticsSummary) -> Self {
        let channels = summary
            .channels
            .iter()
            .map(|c| ChannelMetricsView::from_metrics(c, summary.total_sales))
            .collect();

        Self {
            total_sales: format_currency(summary.total_sales),
            total_net_sales: format_currency(summary.total_net_sales),
            total_orders: summary.total_orders.to_string(),
            total_units: summary.total_units.to_string(),
            average_order_value: format_currency(summary.average_order_value),
            channels,
        }
    }
}

// =============================================================================
// Templates
// =============================================================================

/// Analytics index page template.
#[derive(Template)]
#[template(path = "analytics/index.html")]
pub struct AnalyticsIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub summary: AnalyticsSummaryView,
    pub trend: Vec<DailyMetricsView>,
    pub channels: Vec<SalesChannelView>,
    pub current_range: String,
    pub trend_labels: String,
    pub trend_data: String,
    pub total_expenses: String,
    pub net_income: String,
    pub custom_start: String,
    pub custom_end: String,
}

/// Channels list page template.
#[derive(Template)]
#[template(path = "analytics/channels.html")]
pub struct ChannelsListTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub channels: Vec<SalesChannelView>,
    pub channel_count: i64,
}

/// Channel detail page template.
#[derive(Template)]
#[template(path = "analytics/channel_detail.html")]
pub struct ChannelDetailTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub channel: SalesChannelView,
    pub trend: Vec<DailyMetricsView>,
    pub current_range: String,
}

// =============================================================================
// Attribution View Types
// =============================================================================

/// Per-channel attribution view combining revenue with ad spend.
#[derive(Debug, Clone)]
pub struct ChannelAttributionView {
    pub channel_name: String,
    pub revenue: String,
    pub revenue_raw: f64,
    pub ad_spend: String,
    pub ad_spend_raw: f64,
    pub profit: String,
    pub profit_raw: f64,
    pub roas: String,
    pub orders: String,
    pub pct_of_revenue: String,
    pub is_profitable: bool,
    pub has_ad_spend: bool,
}

/// Attribution summary view.
#[derive(Debug, Clone)]
pub struct AttributionSummaryView {
    pub total_revenue: String,
    pub total_ad_spend: String,
    pub blended_roas: String,
    pub net_channel_profit: String,
    pub channels: Vec<ChannelAttributionView>,
}

/// Attribution page template.
#[derive(Template)]
#[template(path = "analytics/attribution.html")]
struct AttributionTemplate {
    admin_user: AdminUserView,
    current_path: String,
    summary: AttributionSummaryView,
    current_range: String,
    custom_start: String,
    custom_end: String,
    channel_labels: String,
    channel_revenue_data: String,
    trend_labels: String,
    trend_datasets: String,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// Analytics overview page.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AnalyticsQuery>,
) -> Html<String> {
    debug!("Fetching analytics overview page with date range query");
    let date_range = query.to_date_range();

    // Fetch analytics and expense data in parallel
    let (analytics_result, trend_result, channels_result, expenses_total) = tokio::join!(
        state.shopify().get_channel_analytics(&date_range),
        state.shopify().get_channel_trend(None, &date_range),
        state.shopify().get_sales_channels(),
        fetch_expense_total(state.pool(), &query),
    );

    let (summary, raw_total_sales) = process_analytics(analytics_result);
    let (trend, trend_labels, trend_data) = process_trend(trend_result);
    let channels = process_channels(channels_result);
    let net = raw_total_sales - expenses_total;

    let template = AnalyticsIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/analytics".to_string(),
        summary,
        trend,
        channels,
        current_range: query.current_range().to_string(),
        trend_labels,
        trend_data,
        total_expenses: format_currency(expenses_total),
        net_income: format_currency(net),
        custom_start: query.start.clone().unwrap_or_default(),
        custom_end: query.end.clone().unwrap_or_default(),
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {e}");
        "Internal Server Error".to_string()
    }))
}

/// Fetch total expenses for the analytics date range.
async fn fetch_expense_total(pool: &sqlx::PgPool, query: &AnalyticsQuery) -> f64 {
    let now = chrono::Utc::now().date_naive();
    let (start, end) = if let (Some(s), Some(e)) = (&query.start, &query.end) {
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
    };

    let repo = ExpenseRepository::new(pool);
    repo.get_total_expenses(start, end)
        .await
        .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
        .unwrap_or(0.0)
}

/// Process analytics result into summary view and raw total.
fn process_analytics(
    result: Result<AnalyticsSummary, crate::shopify::AdminShopifyError>,
) -> (AnalyticsSummaryView, f64) {
    match result {
        Ok(analytics) => {
            info!(
                total_sales = %analytics.total_sales,
                total_orders = %analytics.total_orders,
                "Successfully fetched channel analytics"
            );
            let total = analytics.total_sales;
            (AnalyticsSummaryView::from(&analytics), total)
        }
        Err(e) => {
            warn!("Failed to fetch channel analytics: {e}");
            (
                AnalyticsSummaryView {
                    total_sales: "$0.00".to_string(),
                    total_net_sales: "$0.00".to_string(),
                    total_orders: "0".to_string(),
                    total_units: "0".to_string(),
                    average_order_value: "$0.00".to_string(),
                    channels: vec![],
                },
                0.0,
            )
        }
    }
}

/// Process trend result into views and Chart.js JSON arrays.
fn process_trend(
    result: Result<Vec<DailyMetrics>, crate::shopify::AdminShopifyError>,
) -> (Vec<DailyMetricsView>, String, String) {
    match result {
        Ok(metrics) => {
            debug!(data_points = %metrics.len(), "Fetched trend data");
            let views: Vec<DailyMetricsView> = metrics.iter().map(DailyMetricsView::from).collect();
            let labels: Vec<String> = metrics
                .iter()
                .map(|d| {
                    chrono::NaiveDate::parse_from_str(&d.date, "%Y-%m-%d")
                        .map_or_else(|_| d.date.clone(), |dt| dt.format("%b %d").to_string())
                })
                .collect();
            let data: Vec<String> = metrics
                .iter()
                .map(|d| format!("{:.2}", d.total_sales))
                .collect();
            let labels_json = serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_string());
            let data_json = format!("[{}]", data.join(","));
            (views, labels_json, data_json)
        }
        Err(e) => {
            warn!("Failed to fetch trend data: {e}");
            (vec![], "[]".to_string(), "[]".to_string())
        }
    }
}

/// Process channels result into views.
fn process_channels(
    result: Result<Vec<SalesChannel>, crate::shopify::AdminShopifyError>,
) -> Vec<SalesChannelView> {
    match result {
        Ok(channels) => {
            debug!(channel_count = %channels.len(), "Fetched sales channels");
            channels.iter().map(SalesChannelView::from).collect()
        }
        Err(e) => {
            warn!("Failed to fetch sales channels: {e}");
            vec![]
        }
    }
}

/// Sales channels list page.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn channels(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
) -> Html<String> {
    debug!("Fetching sales channels list page");

    // Fetch channels and count in parallel
    let (channels_result, count_result) = tokio::join!(
        state.shopify().get_sales_channels(),
        state.shopify().get_sales_channels_count()
    );

    let channels = match channels_result {
        Ok(channels) => {
            info!(channel_count = %channels.len(), "Successfully fetched sales channels");
            channels.iter().map(SalesChannelView::from).collect()
        }
        Err(e) => {
            warn!("Failed to fetch sales channels: {e}");
            vec![]
        }
    };

    let channel_count = count_result.unwrap_or_else(|e| {
        warn!("Failed to fetch channel count: {e}");
        0
    });

    let template = ChannelsListTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/analytics/channels".to_string(),
        channels,
        channel_count,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {e}");
        "Internal Server Error".to_string()
    }))
}

/// Channel detail page - deep-dive for a single channel.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn channel_detail(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(channel_name): Path<String>,
    Query(query): Query<AnalyticsQuery>,
) -> Html<String> {
    debug!(channel = %channel_name, "Fetching channel detail page");
    let date_range = query.to_date_range();

    // Fetch trend data for this specific channel
    let trend_result = state
        .shopify()
        .get_channel_trend(Some(&channel_name), &date_range)
        .await;

    let trend = match trend_result {
        Ok(metrics) => {
            info!(
                channel = %channel_name,
                data_points = %metrics.len(),
                "Successfully fetched channel trend data"
            );
            metrics.iter().map(DailyMetricsView::from).collect()
        }
        Err(e) => {
            warn!(channel = %channel_name, "Failed to fetch channel trend: {e}");
            vec![]
        }
    };

    // Create a view for the channel
    let channel = SalesChannelView {
        id: String::new(),
        name: channel_name.clone(),
        app_title: None,
        auto_publish: false,
    };

    let template = ChannelDetailTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: format!("/analytics/channels/{channel_name}"),
        channel,
        trend,
        current_range: query.current_range().to_string(),
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {e}");
        "Internal Server Error".to_string()
    }))
}

/// Revenue attribution dashboard.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn attribution(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AnalyticsQuery>,
) -> Html<String> {
    debug!("Fetching attribution dashboard");
    let date_range = query.to_date_range();
    let (start, end) = query_to_dates(&query);
    let expense_repo = ExpenseRepository::new(state.pool());

    // Parallel fetch: channel revenue, multi-channel trend, ad spend
    let (analytics_result, trend_result, ad_spend_result) = tokio::join!(
        state.shopify().get_channel_analytics(&date_range),
        state.shopify().get_multi_channel_trend(&date_range),
        expense_repo.get_ad_spend_by_channel(start, end),
    );

    let summary = build_attribution_summary(analytics_result, ad_spend_result);
    let (trend_labels, trend_datasets) = build_multi_channel_trend(trend_result);
    let (channel_labels, channel_revenue_data) = build_channel_chart_data(&summary.channels);

    let template = AttributionTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/analytics/attribution".to_string(),
        summary,
        current_range: query.current_range().to_string(),
        custom_start: query.start.clone().unwrap_or_default(),
        custom_end: query.end.clone().unwrap_or_default(),
        channel_labels,
        channel_revenue_data,
        trend_labels,
        trend_datasets,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {e}");
        "Internal Server Error".to_string()
    }))
}

/// Build attribution summary by joining channel revenue with ad spend.
fn build_attribution_summary(
    analytics_result: Result<AnalyticsSummary, crate::shopify::AdminShopifyError>,
    ad_spend_result: Result<Vec<ChannelAdSpend>, crate::db::RepositoryError>,
) -> AttributionSummaryView {
    let analytics = analytics_result.unwrap_or_default();
    let ad_spend_list = ad_spend_result.unwrap_or_default();

    let ad_spend_map: std::collections::HashMap<String, f64> = ad_spend_list
        .iter()
        .map(|a| {
            (
                a.channel_name.clone(),
                a.total_spend.to_string().parse::<f64>().unwrap_or(0.0),
            )
        })
        .collect();

    let total_revenue = analytics.total_sales;
    let mut total_ad_spend = 0.0;

    let channels: Vec<ChannelAttributionView> = analytics
        .channels
        .iter()
        .map(|ch| {
            let spend = ad_spend_map.get(&ch.channel_name).copied().unwrap_or(0.0);
            total_ad_spend += spend;
            let profit = ch.total_sales - spend;
            let roas = if spend > 0.0 {
                ch.total_sales / spend
            } else {
                0.0
            };
            let pct = if total_revenue > 0.0 {
                (ch.total_sales / total_revenue) * 100.0
            } else {
                0.0
            };

            ChannelAttributionView {
                channel_name: ch.channel_name.clone(),
                revenue: format_currency(ch.total_sales),
                revenue_raw: ch.total_sales,
                ad_spend: if spend > 0.0 {
                    format_currency(spend)
                } else {
                    "\u{2014}".to_string()
                },
                ad_spend_raw: spend,
                profit: format_currency(profit),
                profit_raw: profit,
                roas: if spend > 0.0 {
                    format!("{roas:.1}x")
                } else {
                    "\u{2014}".to_string()
                },
                orders: ch.orders.to_string(),
                pct_of_revenue: format!("{pct:.1}%"),
                is_profitable: profit >= 0.0,
                has_ad_spend: spend > 0.0,
            }
        })
        .collect();

    let net_profit = total_revenue - total_ad_spend;
    let blended = if total_ad_spend > 0.0 {
        format!("{:.1}x", total_revenue / total_ad_spend)
    } else {
        "\u{2014}".to_string()
    };

    AttributionSummaryView {
        total_revenue: format_currency(total_revenue),
        total_ad_spend: format_currency(total_ad_spend),
        blended_roas: blended,
        net_channel_profit: format_currency(net_profit),
        channels,
    }
}

/// Build Chart.js data for the revenue-by-channel doughnut chart.
fn build_channel_chart_data(channels: &[ChannelAttributionView]) -> (String, String) {
    let labels: Vec<&str> = channels.iter().map(|c| c.channel_name.as_str()).collect();
    let data: Vec<String> = channels
        .iter()
        .map(|c| format!("{:.2}", c.revenue_raw))
        .collect();
    let labels_json = serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_string());
    let data_json = format!("[{}]", data.join(","));
    (labels_json, data_json)
}

/// Build Chart.js datasets for the multi-channel stacked area chart.
fn build_multi_channel_trend(
    result: Result<Vec<ChannelDailyMetrics>, crate::shopify::AdminShopifyError>,
) -> (String, String) {
    let metrics = result.unwrap_or_default();
    if metrics.is_empty() {
        return ("[]".to_string(), "[]".to_string());
    }

    // Collect unique dates and channels
    let mut dates: Vec<String> = Vec::new();
    let mut channel_set: Vec<String> = Vec::new();
    for m in &metrics {
        if !dates.contains(&m.date) {
            dates.push(m.date.clone());
        }
        if !channel_set.contains(&m.channel_name) {
            channel_set.push(m.channel_name.clone());
        }
    }

    // Build per-channel data arrays
    let mut datasets = Vec::new();
    let palette = [
        "#d63a2f", "#d4a14a", "#3a9d5c", "#6b8fa3", "#8b5cf6", "#1e4d6e",
    ];
    for (idx, channel) in channel_set.iter().enumerate() {
        let data_points: Vec<String> = dates
            .iter()
            .map(|date| {
                metrics
                    .iter()
                    .find(|m| &m.date == date && &m.channel_name == channel)
                    .map_or_else(|| "0".to_string(), |m| format!("{:.2}", m.total_sales))
            })
            .collect();
        let color = palette
            .get(idx % palette.len())
            .copied()
            .unwrap_or("#6b8fa3");
        datasets.push(format!(
            "{{label:\"{channel}\",data:[{}],borderColor:\"{color}\",backgroundColor:\"{color}20\",fill:true}}",
            data_points.join(",")
        ));
    }

    let labels: Vec<String> = dates
        .iter()
        .map(|d| {
            chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
                .map_or_else(|_| d.clone(), |dt| dt.format("%b %d").to_string())
        })
        .collect();
    let labels_json = serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_string());
    let datasets_json = format!("[{}]", datasets.join(","));

    (labels_json, datasets_json)
}

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

// =============================================================================
// Helper Functions
// =============================================================================

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

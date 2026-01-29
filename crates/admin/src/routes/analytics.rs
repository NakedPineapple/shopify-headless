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
use tracing::instrument;

use crate::{
    filters,
    middleware::auth::RequireAdminAuth,
    shopify::types::{AnalyticsSummary, ChannelMetrics, DailyMetrics, DateRange, SalesChannel},
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
    fn current_range(&self) -> &str {
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
// Route Handlers
// =============================================================================

/// Analytics overview page.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AnalyticsQuery>,
) -> Html<String> {
    let date_range = query.to_date_range();

    // Fetch analytics data in parallel
    let (analytics_result, trend_result, channels_result) = tokio::join!(
        state.shopify().get_channel_analytics(&date_range),
        state.shopify().get_channel_trend(None, &date_range),
        state.shopify().get_sales_channels()
    );

    let summary = match analytics_result {
        Ok(analytics) => AnalyticsSummaryView::from(&analytics),
        Err(e) => {
            tracing::error!("Failed to fetch channel analytics: {e}");
            AnalyticsSummaryView {
                total_sales: "$0.00".to_string(),
                total_net_sales: "$0.00".to_string(),
                total_orders: "0".to_string(),
                total_units: "0".to_string(),
                average_order_value: "$0.00".to_string(),
                channels: vec![],
            }
        }
    };

    let trend = match trend_result {
        Ok(metrics) => metrics.iter().map(DailyMetricsView::from).collect(),
        Err(e) => {
            tracing::error!("Failed to fetch trend data: {e}");
            vec![]
        }
    };

    let channels = match channels_result {
        Ok(channels) => channels.iter().map(SalesChannelView::from).collect(),
        Err(e) => {
            tracing::error!("Failed to fetch sales channels: {e}");
            vec![]
        }
    };

    let template = AnalyticsIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/analytics".to_string(),
        summary,
        trend,
        channels,
        current_range: query.current_range().to_string(),
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {e}");
        "Internal Server Error".to_string()
    }))
}

/// Sales channels list page.
#[instrument(skip(admin, state))]
pub async fn channels(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
) -> Html<String> {
    // Fetch channels and count in parallel
    let (channels_result, count_result) = tokio::join!(
        state.shopify().get_sales_channels(),
        state.shopify().get_sales_channels_count()
    );

    let channels = match channels_result {
        Ok(channels) => channels.iter().map(SalesChannelView::from).collect(),
        Err(e) => {
            tracing::error!("Failed to fetch sales channels: {e}");
            vec![]
        }
    };

    let channel_count = count_result.unwrap_or_else(|e| {
        tracing::error!("Failed to fetch channel count: {e}");
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
#[instrument(skip(admin, state))]
pub async fn channel_detail(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(channel_name): Path<String>,
    Query(query): Query<AnalyticsQuery>,
) -> Html<String> {
    let date_range = query.to_date_range();

    // Fetch trend data for this specific channel
    let trend_result = state
        .shopify()
        .get_channel_trend(Some(&channel_name), &date_range)
        .await;

    let trend = match trend_result {
        Ok(metrics) => metrics.iter().map(DailyMetricsView::from).collect(),
        Err(e) => {
            tracing::error!("Failed to fetch channel trend: {e}");
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

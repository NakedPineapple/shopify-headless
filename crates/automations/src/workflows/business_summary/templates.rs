//! Askama template rendering for business summary emails.

use askama::Template;
use rust_decimal::Decimal;

use crate::shopify::analytics::{ChannelMetrics, LowStockItem, ProductRevenue};

use super::types::{DailySummaryData, WeeklySummaryData};

// =============================================================================
// Daily Summary Templates
// =============================================================================

#[derive(Template)]
#[template(path = "email/daily_summary.html")]
struct DailyHtmlTemplate<'a> {
    date: &'a str,
    revenue: String,
    orders: i64,
    units: i64,
    aov: String,
    top_products: &'a [ProductRevenue],
    channels: &'a [ChannelMetrics],
    low_stock_items: &'a [LowStockItem],
}

#[derive(Template)]
#[template(path = "email/daily_summary.txt")]
struct DailyTextTemplate<'a> {
    date: &'a str,
    revenue: String,
    orders: i64,
    units: i64,
    aov: String,
    top_products: &'a [ProductRevenue],
    channels: &'a [ChannelMetrics],
    low_stock_items: &'a [LowStockItem],
}

/// Render the daily summary as `(html, plain_text)`.
pub fn render_daily(data: &DailySummaryData) -> (String, String) {
    let html_tmpl = DailyHtmlTemplate {
        date: &data.date,
        revenue: format_currency(data.revenue),
        orders: data.orders,
        units: data.units,
        aov: format_currency(data.aov),
        top_products: &data.top_products,
        channels: &data.channels,
        low_stock_items: &data.low_stock_items,
    };

    let text_tmpl = DailyTextTemplate {
        date: &data.date,
        revenue: format_currency(data.revenue),
        orders: data.orders,
        units: data.units,
        aov: format_currency(data.aov),
        top_products: &data.top_products,
        channels: &data.channels,
        low_stock_items: &data.low_stock_items,
    };

    (
        html_tmpl.render().unwrap_or_default(),
        text_tmpl.render().unwrap_or_default(),
    )
}

// =============================================================================
// Weekly Summary Templates
// =============================================================================

#[derive(Template)]
#[template(path = "email/weekly_summary.html")]
struct WeeklyHtmlTemplate<'a> {
    week_start: &'a str,
    week_end: &'a str,
    revenue: String,
    orders: i64,
    units: i64,
    aov: String,
    prev_revenue: String,
    prev_orders: i64,
    revenue_change: String,
    orders_change: String,
    aov_change: String,
    top_products: &'a [ProductRevenue],
    channels: &'a [ChannelMetrics],
    total_expenses: String,
    ad_spend: String,
    ad_channels: &'a [crate::db::expense::ChannelAdSpend],
    gross_profit: String,
    gross_margin: String,
    net_profit: String,
    net_margin: String,
    low_stock_items: &'a [LowStockItem],
}

#[derive(Template)]
#[template(path = "email/weekly_summary.txt")]
struct WeeklyTextTemplate<'a> {
    week_start: &'a str,
    week_end: &'a str,
    revenue: String,
    orders: i64,
    units: i64,
    aov: String,
    prev_revenue: String,
    prev_orders: i64,
    revenue_change: String,
    orders_change: String,
    aov_change: String,
    top_products: &'a [ProductRevenue],
    channels: &'a [ChannelMetrics],
    total_expenses: String,
    ad_spend: String,
    ad_channels: &'a [crate::db::expense::ChannelAdSpend],
    gross_profit: String,
    gross_margin: String,
    net_profit: String,
    net_margin: String,
    low_stock_items: &'a [LowStockItem],
}

/// Render the weekly summary as `(html, plain_text)`.
pub fn render_weekly(data: &WeeklySummaryData) -> (String, String) {
    let html_tmpl = WeeklyHtmlTemplate {
        week_start: &data.week_start,
        week_end: &data.week_end,
        revenue: format_currency(data.current.revenue),
        orders: data.current.orders,
        units: data.current.units,
        aov: format_currency(data.current.aov),
        prev_revenue: format_currency(data.previous.revenue),
        prev_orders: data.previous.orders,
        revenue_change: format_pct(data.comparison.revenue),
        orders_change: format_pct(data.comparison.orders),
        aov_change: format_pct(data.comparison.aov),
        top_products: &data.top_products,
        channels: &data.channels,
        total_expenses: format_currency(data.expenses.total),
        ad_spend: format_currency(data.expenses.ad_spend_total),
        ad_channels: &data.expenses.by_channel,
        gross_profit: format_currency(data.profit.gross_profit),
        gross_margin: format!("{:.1}%", data.profit.gross_margin_pct),
        net_profit: format_currency(data.profit.net_profit),
        net_margin: format!("{:.1}%", data.profit.net_margin_pct),
        low_stock_items: &data.low_stock_items,
    };

    let text_tmpl = WeeklyTextTemplate {
        week_start: &data.week_start,
        week_end: &data.week_end,
        revenue: format_currency(data.current.revenue),
        orders: data.current.orders,
        units: data.current.units,
        aov: format_currency(data.current.aov),
        prev_revenue: format_currency(data.previous.revenue),
        prev_orders: data.previous.orders,
        revenue_change: format_pct(data.comparison.revenue),
        orders_change: format_pct(data.comparison.orders),
        aov_change: format_pct(data.comparison.aov),
        top_products: &data.top_products,
        channels: &data.channels,
        total_expenses: format_currency(data.expenses.total),
        ad_spend: format_currency(data.expenses.ad_spend_total),
        ad_channels: &data.expenses.by_channel,
        gross_profit: format_currency(data.profit.gross_profit),
        gross_margin: format!("{:.1}%", data.profit.gross_margin_pct),
        net_profit: format_currency(data.profit.net_profit),
        net_margin: format!("{:.1}%", data.profit.net_margin_pct),
        low_stock_items: &data.low_stock_items,
    };

    (
        html_tmpl.render().unwrap_or_default(),
        text_tmpl.render().unwrap_or_default(),
    )
}

// =============================================================================
// Formatting Helpers
// =============================================================================

fn format_currency(amount: Decimal) -> String {
    let rounded = amount.round_dp(2);
    format!("${rounded}")
}

fn format_pct(value: f64) -> String {
    let arrow = if value > 0.0 {
        "\u{2191}"
    } else if value < 0.0 {
        "\u{2193}"
    } else {
        ""
    };
    format!("{arrow}{:.1}%", value.abs())
}

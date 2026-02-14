//! Data types for business summary emails.

use rust_decimal::Decimal;

use crate::shopify::analytics::{ChannelMetrics, LowStockItem, ProductRevenue};

/// Data for the daily business summary email.
#[derive(Debug, Clone)]
pub struct DailySummaryData {
    /// Date being summarized (formatted for display, e.g. "February 12, 2026").
    pub date: String,
    /// Total revenue for the day.
    pub revenue: Decimal,
    /// Total orders for the day.
    pub orders: i64,
    /// Total units sold.
    pub units: i64,
    /// Average order value.
    pub aov: Decimal,
    /// Top products by revenue.
    pub top_products: Vec<ProductRevenue>,
    /// Sales by channel.
    pub channels: Vec<ChannelMetrics>,
    /// Products below the low stock threshold.
    pub low_stock_items: Vec<LowStockItem>,
}

/// Data for the weekly business summary email.
#[derive(Debug, Clone)]
pub struct WeeklySummaryData {
    /// Start of the week (formatted for display).
    pub week_start: String,
    /// End of the week (formatted for display).
    pub week_end: String,
    /// Current week metrics.
    pub current: PeriodMetrics,
    /// Previous week metrics (for comparison).
    pub previous: PeriodMetrics,
    /// Week-over-week comparison.
    pub comparison: ComparisonMetrics,
    /// Sales by channel.
    pub channels: Vec<ChannelMetrics>,
    /// Expense breakdown.
    pub expenses: ExpenseSummary,
    /// Profit calculations.
    pub profit: ProfitSummary,
    /// Top products by revenue.
    pub top_products: Vec<ProductRevenue>,
    /// Products below the low stock threshold.
    pub low_stock_items: Vec<LowStockItem>,
}

/// Aggregate metrics for a time period.
#[derive(Debug, Clone, Default)]
pub struct PeriodMetrics {
    pub revenue: Decimal,
    pub orders: i64,
    pub units: i64,
    pub aov: Decimal,
}

/// Week-over-week comparison percentages.
#[derive(Debug, Clone, Default)]
pub struct ComparisonMetrics {
    /// Revenue change as a percentage.
    pub revenue: f64,
    /// Order count change as a percentage.
    pub orders: f64,
    /// AOV change as a percentage.
    pub aov: f64,
}

/// Expense breakdown for the weekly summary.
#[derive(Debug, Clone, Default)]
pub struct ExpenseSummary {
    /// Total expenses for the period.
    pub total: Decimal,
    /// Total advertising spend.
    pub ad_spend_total: Decimal,
    /// Ad spend broken down by channel.
    pub by_channel: Vec<crate::db::expense::ChannelAdSpend>,
    /// Expenses broken down by category type (advertising, saas, shipping, etc.).
    pub by_category: Vec<crate::db::expense::ExpenseCategorySummary>,
}

/// Profit margin calculations for the weekly summary.
#[derive(Debug, Clone, Default)]
pub struct ProfitSummary {
    /// Revenue minus COGS.
    pub gross_profit: Decimal,
    /// Gross profit as a percentage of revenue.
    pub gross_margin_pct: f64,
    /// Revenue minus COGS minus expenses.
    pub net_profit: Decimal,
    /// Net profit as a percentage of revenue.
    pub net_margin_pct: f64,
}

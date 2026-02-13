//! Data collection for daily and weekly business summaries.
//!
//! Queries Shopify analytics and the `np_admin` database to assemble
//! the data needed to render summary email templates.

use chrono::{Datelike, Duration, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::warn;

use crate::db::{cogs, expense};
use crate::shopify::analytics;
use crate::shopify::client::ShopifyClient;

use super::types::{
    ComparisonMetrics, DailySummaryData, ExpenseSummary, PeriodMetrics, ProfitSummary,
    WeeklySummaryData,
};

/// Collect data for the daily summary email.
///
/// Queries yesterday's sales from Shopify and current inventory levels.
pub async fn collect_daily_data(
    shopify: &ShopifyClient,
    low_stock_threshold: i32,
) -> Result<DailySummaryData, String> {
    let yesterday = (Utc::now() - Duration::days(1)).date_naive();
    let since = yesterday.format("%Y-%m-%d").to_string();
    let until = yesterday.format("%Y-%m-%d").to_string();
    let date_display = yesterday.format("%B %-d, %Y").to_string();

    let metrics = analytics::get_summary_analytics(shopify, &since, &until)
        .await
        .map_err(|e| format!("failed to fetch daily analytics: {e}"))?;

    let top_products = analytics::get_top_products(shopify, &since, &until, 5)
        .await
        .map_err(|e| format!("failed to fetch top products: {e}"))?;

    let channels = analytics::get_channel_breakdown(shopify, &since, &until)
        .await
        .map_err(|e| format!("failed to fetch channel breakdown: {e}"))?;

    let low_stock_items = analytics::get_low_stock_items(shopify, low_stock_threshold)
        .await
        .unwrap_or_else(|e| {
            warn!(error = %e, "failed to fetch low stock items for daily summary");
            Vec::new()
        });

    Ok(DailySummaryData {
        date: date_display,
        revenue: metrics.total_revenue,
        orders: metrics.total_orders,
        units: metrics.total_units,
        aov: metrics.average_order_value,
        top_products,
        channels,
        low_stock_items,
    })
}

/// Collect data for the weekly summary email.
///
/// Queries this week vs. last week from Shopify, plus expenses, COGS, and
/// profit margins from the admin database.
pub async fn collect_weekly_data(
    pool: &PgPool,
    shopify: &ShopifyClient,
    low_stock_threshold: i32,
) -> Result<WeeklySummaryData, String> {
    let today = Utc::now().date_naive();
    // Week = Mon–Sun. Find last Monday.
    let days_since_monday = today.weekday().num_days_from_monday();
    let this_monday = today - Duration::days(i64::from(days_since_monday));
    let last_monday = this_monday - Duration::days(7);
    let last_sunday = this_monday - Duration::days(1);
    let prev_monday = last_monday - Duration::days(7);
    let prev_sunday = last_monday - Duration::days(1);

    let current = fetch_period_metrics(shopify, last_monday, last_sunday).await?;
    let previous = fetch_period_metrics(shopify, prev_monday, prev_sunday).await?;
    let comparison = compute_comparison(&current, &previous);

    let channels = analytics::get_channel_breakdown(
        shopify,
        &last_monday.format("%Y-%m-%d").to_string(),
        &last_sunday.format("%Y-%m-%d").to_string(),
    )
    .await
    .map_err(|e| format!("failed to fetch weekly channel breakdown: {e}"))?;

    let top_products = analytics::get_top_products(
        shopify,
        &last_monday.format("%Y-%m-%d").to_string(),
        &last_sunday.format("%Y-%m-%d").to_string(),
        10,
    )
    .await
    .map_err(|e| format!("failed to fetch weekly top products: {e}"))?;

    let expenses = collect_expenses(pool, last_monday, last_sunday).await;
    let profit = compute_profit(pool, &current, &expenses, last_monday, last_sunday).await;

    let low_stock_items = analytics::get_low_stock_items(shopify, low_stock_threshold)
        .await
        .unwrap_or_else(|e| {
            warn!(error = %e, "failed to fetch low stock items for weekly summary");
            Vec::new()
        });

    Ok(WeeklySummaryData {
        week_start: last_monday.format("%B %-d").to_string(),
        week_end: last_sunday.format("%B %-d, %Y").to_string(),
        current,
        previous,
        comparison,
        channels,
        expenses,
        profit,
        top_products,
        low_stock_items,
    })
}

async fn fetch_period_metrics(
    shopify: &ShopifyClient,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<PeriodMetrics, String> {
    let since = start.format("%Y-%m-%d").to_string();
    let until = end.format("%Y-%m-%d").to_string();

    let m = analytics::get_summary_analytics(shopify, &since, &until)
        .await
        .map_err(|e| format!("failed to fetch period analytics: {e}"))?;

    Ok(PeriodMetrics {
        revenue: m.total_revenue,
        orders: m.total_orders,
        units: m.total_units,
        aov: m.average_order_value,
    })
}

fn compute_comparison(current: &PeriodMetrics, previous: &PeriodMetrics) -> ComparisonMetrics {
    ComparisonMetrics {
        revenue: pct_change(previous.revenue, current.revenue),
        orders: pct_change(
            Decimal::from(previous.orders),
            Decimal::from(current.orders),
        ),
        aov: pct_change(previous.aov, current.aov),
    }
}

fn pct_change(old: Decimal, new: Decimal) -> f64 {
    if old.is_zero() {
        if new.is_zero() { 0.0 } else { 100.0 }
    } else {
        let diff = new - old;
        // Decimal arithmetic, convert to f64 only at the end
        let pct = diff * Decimal::from(100) / old;
        pct.to_string().parse::<f64>().unwrap_or(0.0)
    }
}

async fn collect_expenses(pool: &PgPool, start: NaiveDate, end: NaiveDate) -> ExpenseSummary {
    let total = expense::get_total_expenses(pool, start, end)
        .await
        .unwrap_or_else(|e| {
            warn!(error = %e, "failed to fetch total expenses for summary");
            Decimal::ZERO
        });

    let by_channel = expense::get_ad_spend_by_channel(pool, start, end)
        .await
        .unwrap_or_else(|e| {
            warn!(error = %e, "failed to fetch ad spend for summary");
            Vec::new()
        });

    let ad_spend_total = by_channel.iter().map(|c| c.total_spend).sum();

    ExpenseSummary {
        total,
        ad_spend_total,
        by_channel,
    }
}

async fn compute_profit(
    pool: &PgPool,
    current: &PeriodMetrics,
    expenses: &ExpenseSummary,
    start: NaiveDate,
    end: NaiveDate,
) -> ProfitSummary {
    let total_cogs = cogs::get_total_cogs(pool, start, end)
        .await
        .unwrap_or_else(|e| {
            warn!(error = %e, "failed to fetch COGS for summary");
            Decimal::ZERO
        });

    let gross_profit = current.revenue - total_cogs;
    let net_profit = gross_profit - expenses.total;

    let revenue_f64 = current.revenue.to_string().parse::<f64>().unwrap_or(0.0);

    let gross_margin_pct = if revenue_f64 > 0.0 {
        let gp = gross_profit.to_string().parse::<f64>().unwrap_or(0.0);
        (gp / revenue_f64) * 100.0
    } else {
        0.0
    };

    let net_margin_pct = if revenue_f64 > 0.0 {
        let np = net_profit.to_string().parse::<f64>().unwrap_or(0.0);
        (np / revenue_f64) * 100.0
    } else {
        0.0
    };

    ProfitSummary {
        gross_profit,
        gross_margin_pct,
        net_profit,
        net_margin_pct,
    }
}

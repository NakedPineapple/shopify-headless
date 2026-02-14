//! Expense queries for business summary emails.
//!
//! Simplified expense aggregation queries from the `np_admin` database,
//! tailored for inclusion in summary email content.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, instrument};

use super::RepositoryError;

/// Convert chrono `NaiveDate` to `time::Date` for `SQLx` bind compatibility.
///
/// See `crates/admin/src/db/inventory_lot.rs` for the full explanation.
fn to_time_date(date: NaiveDate) -> time::Date {
    use chrono::Datelike;
    let month = u8::try_from(date.month()).expect("month in range 1-12");
    let day = u8::try_from(date.day()).expect("day in range 1-31");
    time::Date::from_calendar_date(
        date.year(),
        time::Month::try_from(month).expect("valid month"),
        day,
    )
    .expect("valid date")
}

/// Total expenses for a date range.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
#[instrument(skip(pool), level = "debug")]
pub async fn get_total_expenses(
    pool: &PgPool,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Decimal, RepositoryError> {
    debug!("calculating total expenses for summary");
    let total = sqlx::query_scalar!(
        r#"
        SELECT COALESCE(SUM(amount), 0) as "total!: Decimal"
        FROM admin.expense
        WHERE expense_date >= $1 AND expense_date <= $2
        "#,
        to_time_date(start),
        to_time_date(end)
    )
    .fetch_one(pool)
    .await?;

    Ok(total)
}

/// Ad spend by channel for a date range.
#[derive(Debug, Clone)]
pub struct ChannelAdSpend {
    pub channel_name: String,
    pub total_spend: Decimal,
}

/// Internal row type for the ad-spend query.
#[derive(Debug, sqlx::FromRow)]
struct ChannelAdSpendRow {
    channel_name: String,
    total_spend: Decimal,
}

/// Expenses grouped by category type for a date range.
#[derive(Debug, Clone)]
pub struct ExpenseCategorySummary {
    pub expense_type: String,
    pub total_amount: Decimal,
}

/// Internal row type for the category query.
#[derive(Debug, sqlx::FromRow)]
struct ExpenseCategoryRow {
    expense_type: String,
    total_amount: Decimal,
}

/// Get expenses grouped by category type for a date range.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
#[instrument(skip(pool), level = "debug")]
pub async fn get_expenses_by_category(
    pool: &PgPool,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<ExpenseCategorySummary>, RepositoryError> {
    debug!("aggregating expenses by category for summary");
    let rows = sqlx::query_as!(
        ExpenseCategoryRow,
        r#"
        SELECT
            c.expense_type::TEXT as "expense_type!",
            COALESCE(SUM(e.amount), 0) as "total_amount!: Decimal"
        FROM admin.expense e
        INNER JOIN admin.expense_category c ON c.id = e.category_id
        WHERE
            e.expense_date >= $1
            AND e.expense_date <= $2
        GROUP BY c.expense_type
        ORDER BY SUM(e.amount) DESC
        "#,
        to_time_date(start),
        to_time_date(end)
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ExpenseCategorySummary {
            expense_type: r.expense_type,
            total_amount: r.total_amount,
        })
        .collect())
}

/// Get ad spend grouped by channel for a date range.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
#[instrument(skip(pool), level = "debug")]
pub async fn get_ad_spend_by_channel(
    pool: &PgPool,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<ChannelAdSpend>, RepositoryError> {
    debug!("aggregating ad spend by channel for summary");
    let rows = sqlx::query_as!(
        ChannelAdSpendRow,
        r#"
        SELECT
            e.channel_name as "channel_name!",
            COALESCE(SUM(e.amount), 0) as "total_spend!: Decimal"
        FROM admin.expense e
        INNER JOIN admin.expense_category c ON c.id = e.category_id
        WHERE
            e.expense_date >= $1
            AND e.expense_date <= $2
            AND e.channel_name IS NOT NULL
            AND c.expense_type = 'advertising'
        GROUP BY e.channel_name
        ORDER BY SUM(e.amount) DESC
        "#,
        to_time_date(start),
        to_time_date(end)
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ChannelAdSpend {
            channel_name: r.channel_name,
            total_spend: r.total_spend,
        })
        .collect())
}

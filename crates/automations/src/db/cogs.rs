//! COGS (cost of goods sold) queries for business summary emails.
//!
//! Aggregates lot allocation data from the `np_admin` database to compute
//! total COGS for a given period, used in weekly profit margin calculations.

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, instrument};

use super::RepositoryError;

/// Convert a `NaiveDate` to `time::OffsetDateTime` at midnight UTC.
fn to_time_offset_midnight(date: NaiveDate) -> time::OffsetDateTime {
    let d = time::Date::from_calendar_date(
        date.year(),
        time::Month::try_from(u8::try_from(date.month()).expect("month in range"))
            .expect("valid month"),
        u8::try_from(date.day()).expect("day in range"),
    )
    .expect("valid date");
    time::OffsetDateTime::new_utc(d, time::Time::MIDNIGHT)
}

/// Convert a `NaiveDate` to `time::OffsetDateTime` at the start of the next day.
fn to_time_offset_next_midnight(date: NaiveDate) -> time::OffsetDateTime {
    let next = date
        .succ_opt()
        .expect("date has a successor within valid range");
    to_time_offset_midnight(next)
}

/// Get total COGS for a date range.
///
/// Uses lot allocation timestamps to determine which costs fall within the range.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
#[instrument(skip(pool), level = "debug")]
pub async fn get_total_cogs(
    pool: &PgPool,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Decimal, RepositoryError> {
    debug!("calculating total COGS for summary");

    let start_ts = to_time_offset_midnight(start);
    let end_exclusive = to_time_offset_next_midnight(end);

    let total = sqlx::query_scalar!(
        r#"
        SELECT COALESCE(SUM(la.quantity::numeric * mb.cost_per_unit), 0) as "total!: Decimal"
        FROM admin.lot_allocation la
        JOIN admin.inventory_lot il ON la.lot_id = il.id
        JOIN admin.manufacturing_batch mb ON il.batch_id = mb.id
        WHERE la.allocated_at >= $1
          AND la.allocated_at < $2
        "#,
        start_ts,
        end_exclusive,
    )
    .fetch_one(pool)
    .await?;

    Ok(total)
}

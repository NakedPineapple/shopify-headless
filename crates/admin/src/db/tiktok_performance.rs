//! TikTok Shop performance metrics repository.
//!
//! Tracks seller performance snapshots including delivery rates,
//! cancellation rates, and overall shop health scores.

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, instrument};

use super::RepositoryError;

/// Convert chrono `NaiveDate` to `time::Date` for `SQLx` bind params.
///
/// See `crates/admin/src/db/inventory_lot.rs` for documentation on why this is needed.
fn to_time_date(date: NaiveDate) -> time::Date {
    let month = u8::try_from(date.month()).expect("month in range 1-12");
    let day = u8::try_from(date.day()).expect("day in range 1-31");
    time::Date::from_calendar_date(
        date.year(),
        time::Month::try_from(month).expect("valid month"),
        day,
    )
    .expect("valid date")
}

// =============================================================================
// Types
// =============================================================================

/// A TikTok Shop performance snapshot for a given date.
#[derive(Debug, Clone)]
pub struct TikTokPerformanceSnapshot {
    pub id: i32,
    pub snapshot_date: NaiveDate,
    pub on_time_delivery_rate: Option<Decimal>,
    pub late_dispatch_rate: Option<Decimal>,
    pub seller_fault_cancel_rate: Option<Decimal>,
    pub customer_satisfaction_rate: Option<Decimal>,
    pub overall_health: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct SnapshotRow {
    id: i32,
    snapshot_date: NaiveDate,
    on_time_delivery_rate: Option<Decimal>,
    late_dispatch_rate: Option<Decimal>,
    seller_fault_cancel_rate: Option<Decimal>,
    customer_satisfaction_rate: Option<Decimal>,
    overall_health: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<SnapshotRow> for TikTokPerformanceSnapshot {
    fn from(row: SnapshotRow) -> Self {
        Self {
            id: row.id,
            snapshot_date: row.snapshot_date,
            on_time_delivery_rate: row.on_time_delivery_rate,
            late_dispatch_rate: row.late_dispatch_rate,
            seller_fault_cancel_rate: row.seller_fault_cancel_rate,
            customer_satisfaction_rate: row.customer_satisfaction_rate,
            overall_health: row.overall_health,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Parameters for upserting a performance snapshot.
pub struct UpsertTikTokPerformanceParams {
    pub snapshot_date: NaiveDate,
    pub on_time_delivery_rate: Option<Decimal>,
    pub late_dispatch_rate: Option<Decimal>,
    pub seller_fault_cancel_rate: Option<Decimal>,
    pub customer_satisfaction_rate: Option<Decimal>,
    pub overall_health: String,
}

impl std::fmt::Debug for UpsertTikTokPerformanceParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpsertTikTokPerformanceParams")
            .field("snapshot_date", &self.snapshot_date)
            .field("overall_health", &self.overall_health)
            .finish_non_exhaustive()
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for TikTok performance snapshot database operations.
pub struct TikTokPerformanceRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> TikTokPerformanceRepository<'a> {
    /// Create a new TikTok performance repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Upsert a performance snapshot for a date.
    ///
    /// Uses upsert on `snapshot_date` to handle both new and existing snapshots.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(date = %params.snapshot_date), level = "debug")]
    pub async fn upsert_snapshot(
        &self,
        params: &UpsertTikTokPerformanceParams,
    ) -> Result<i32, RepositoryError> {
        debug!("Upserting TikTok performance snapshot");

        let snapshot_date = to_time_date(params.snapshot_date);
        let otd_rate = params.on_time_delivery_rate.map(|d| d.to_string());
        let ld_rate = params.late_dispatch_rate.map(|d| d.to_string());
        let sfc_rate = params.seller_fault_cancel_rate.map(|d| d.to_string());
        let cs_rate = params.customer_satisfaction_rate.map(|d| d.to_string());

        let row = sqlx::query_scalar!(
            r#"
            INSERT INTO admin.tiktok_shop_performance (
                snapshot_date,
                on_time_delivery_rate,
                late_dispatch_rate,
                seller_fault_cancel_rate,
                customer_satisfaction_rate,
                overall_health
            )
            VALUES (
                $1,
                $2::text::numeric,
                $3::text::numeric,
                $4::text::numeric,
                $5::text::numeric,
                $6
            )
            ON CONFLICT (snapshot_date) DO UPDATE SET
                on_time_delivery_rate = EXCLUDED.on_time_delivery_rate,
                late_dispatch_rate = EXCLUDED.late_dispatch_rate,
                seller_fault_cancel_rate = EXCLUDED.seller_fault_cancel_rate,
                customer_satisfaction_rate = EXCLUDED.customer_satisfaction_rate,
                overall_health = EXCLUDED.overall_health,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            RETURNING id as "id!"
            "#,
            snapshot_date,
            otd_rate,
            ld_rate,
            sfc_rate,
            cs_rate,
            &params.overall_health
        )
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    /// Get the most recent performance snapshot.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_latest(&self) -> Result<Option<TikTokPerformanceSnapshot>, RepositoryError> {
        debug!("Fetching latest TikTok performance snapshot");

        let row = sqlx::query_as!(
            SnapshotRow,
            r#"
            SELECT
                id,
                snapshot_date as "snapshot_date: NaiveDate",
                on_time_delivery_rate as "on_time_delivery_rate: Decimal",
                late_dispatch_rate as "late_dispatch_rate: Decimal",
                seller_fault_cancel_rate as "seller_fault_cancel_rate: Decimal",
                customer_satisfaction_rate as "customer_satisfaction_rate: Decimal",
                overall_health,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.tiktok_shop_performance
            ORDER BY snapshot_date DESC
            LIMIT 1
            "#
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(TikTokPerformanceSnapshot::from))
    }

    /// Get performance snapshot history, most recent first.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_history(
        &self,
        limit: i64,
    ) -> Result<Vec<TikTokPerformanceSnapshot>, RepositoryError> {
        debug!("Fetching TikTok performance snapshot history");

        let rows = sqlx::query_as!(
            SnapshotRow,
            r#"
            SELECT
                id,
                snapshot_date as "snapshot_date: NaiveDate",
                on_time_delivery_rate as "on_time_delivery_rate: Decimal",
                late_dispatch_rate as "late_dispatch_rate: Decimal",
                seller_fault_cancel_rate as "seller_fault_cancel_rate: Decimal",
                customer_satisfaction_rate as "customer_satisfaction_rate: Decimal",
                overall_health,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.tiktok_shop_performance
            ORDER BY snapshot_date DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(TikTokPerformanceSnapshot::from)
            .collect())
    }
}

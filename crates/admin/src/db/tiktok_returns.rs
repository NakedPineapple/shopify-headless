//! TikTok Shop returns repository.
//!
//! Caches TikTok Shop return/refund requests locally.
//! Returns are upserted by `return_id`.

use chrono::{DateTime, Datelike, Timelike, Utc};
use sqlx::PgPool;
use tracing::{debug, info, instrument};

use super::RepositoryError;

/// Convert chrono `DateTime<Utc>` to `time::OffsetDateTime` for `SQLx` bind params.
///
/// See `crates/automations/src/db/mod.rs` for documentation on why this is needed.
fn to_time_offset(dt: DateTime<Utc>) -> time::OffsetDateTime {
    let date = time::Date::from_calendar_date(
        dt.year(),
        time::Month::try_from(u8::try_from(dt.month()).expect("month in range"))
            .expect("valid month"),
        u8::try_from(dt.day()).expect("day in range"),
    )
    .expect("valid date");
    let t = time::Time::from_hms_nano(
        u8::try_from(dt.hour()).expect("hour in range"),
        u8::try_from(dt.minute()).expect("minute in range"),
        u8::try_from(dt.second()).expect("second in range"),
        dt.timestamp_subsec_nanos(),
    )
    .expect("valid time");
    time::OffsetDateTime::new_utc(date, t)
}

// =============================================================================
// Types
// =============================================================================

/// A cached TikTok Shop return/refund request.
#[derive(Debug, Clone)]
pub struct CachedTikTokReturn {
    pub id: i32,
    pub return_id: String,
    pub tiktok_order_id: Option<String>,
    pub return_status: String,
    pub return_type: Option<String>,
    pub reason: Option<String>,
    pub buyer_note: Option<String>,
    pub refund_amount: Option<String>,
    pub currency: Option<String>,
    pub decision_deadline: Option<DateTime<Utc>>,
    pub return_tracking_number: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct ReturnRow {
    id: i32,
    return_id: String,
    tiktok_order_id: Option<String>,
    return_status: String,
    return_type: Option<String>,
    reason: Option<String>,
    buyer_note: Option<String>,
    refund_amount: Option<String>,
    currency: Option<String>,
    decision_deadline: Option<DateTime<Utc>>,
    return_tracking_number: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ReturnRow> for CachedTikTokReturn {
    fn from(row: ReturnRow) -> Self {
        Self {
            id: row.id,
            return_id: row.return_id,
            tiktok_order_id: row.tiktok_order_id,
            return_status: row.return_status,
            return_type: row.return_type,
            reason: row.reason,
            buyer_note: row.buyer_note,
            refund_amount: row.refund_amount,
            currency: row.currency,
            decision_deadline: row.decision_deadline,
            return_tracking_number: row.return_tracking_number,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Parameters for upserting a TikTok return.
pub struct UpsertTikTokReturnParams<'a> {
    pub return_id: &'a str,
    pub tiktok_order_id: Option<&'a str>,
    pub return_status: &'a str,
    pub return_type: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub buyer_note: Option<&'a str>,
    pub refund_amount: Option<&'a str>,
    pub currency: Option<&'a str>,
    pub decision_deadline: Option<DateTime<Utc>>,
    pub return_tracking_number: Option<&'a str>,
}

impl std::fmt::Debug for UpsertTikTokReturnParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpsertTikTokReturnParams")
            .field("return_id", &self.return_id)
            .field("tiktok_order_id", &self.tiktok_order_id)
            .field("return_status", &self.return_status)
            .finish_non_exhaustive()
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for TikTok return database operations.
pub struct TikTokReturnRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> TikTokReturnRepository<'a> {
    /// Create a new TikTok return repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// List returns with pagination and optional status filter.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn list(
        &self,
        limit: i64,
        offset: i64,
        status_filter: Option<&str>,
    ) -> Result<Vec<CachedTikTokReturn>, RepositoryError> {
        debug!("Listing TikTok returns");

        let rows = sqlx::query_as!(
            ReturnRow,
            r#"
            SELECT
                id, return_id, tiktok_order_id,
                return_status, return_type,
                reason, buyer_note,
                refund_amount, currency,
                decision_deadline as "decision_deadline: DateTime<Utc>",
                return_tracking_number,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.tiktok_returns
            WHERE ($3::text IS NULL OR return_status = $3)
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset,
            status_filter
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(CachedTikTokReturn::from).collect())
    }

    /// Count returns with optional status filter.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn count(&self, status_filter: Option<&str>) -> Result<i64, RepositoryError> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM admin.tiktok_returns
            WHERE ($1::text IS NULL OR return_status = $1)
            "#,
            status_filter
        )
        .fetch_one(self.pool)
        .await?;

        Ok(count)
    }

    /// Get a single return by database ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_by_id(&self, id: i32) -> Result<Option<CachedTikTokReturn>, RepositoryError> {
        debug!("Fetching TikTok return by ID");

        let row = sqlx::query_as!(
            ReturnRow,
            r#"
            SELECT
                id, return_id, tiktok_order_id,
                return_status, return_type,
                reason, buyer_note,
                refund_amount, currency,
                decision_deadline as "decision_deadline: DateTime<Utc>",
                return_tracking_number,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.tiktok_returns
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(CachedTikTokReturn::from))
    }

    /// Upsert a return (insert or update on conflict).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(return_id = %params.return_id), level = "debug")]
    pub async fn upsert(
        &self,
        params: &UpsertTikTokReturnParams<'_>,
    ) -> Result<i32, RepositoryError> {
        let decision_deadline = params.decision_deadline.map(to_time_offset);

        let row = sqlx::query_scalar!(
            r#"
            INSERT INTO admin.tiktok_returns (
                return_id, tiktok_order_id, return_status,
                return_type, reason, buyer_note,
                refund_amount, currency,
                decision_deadline, return_tracking_number
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (return_id) DO UPDATE SET
                return_status = EXCLUDED.return_status,
                reason = EXCLUDED.reason,
                buyer_note = EXCLUDED.buyer_note,
                refund_amount = EXCLUDED.refund_amount,
                decision_deadline = EXCLUDED.decision_deadline,
                return_tracking_number = EXCLUDED.return_tracking_number,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            RETURNING id as "id!"
            "#,
            params.return_id,
            params.tiktok_order_id,
            params.return_status,
            params.return_type,
            params.reason,
            params.buyer_note,
            params.refund_amount,
            params.currency,
            decision_deadline,
            params.return_tracking_number
        )
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    /// Update only the status of a return.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(return_id = %return_id, new_status = %new_status), level = "debug")]
    pub async fn update_status(
        &self,
        return_id: &str,
        new_status: &str,
    ) -> Result<bool, RepositoryError> {
        debug!("Updating TikTok return status");

        let result = sqlx::query!(
            r#"
            UPDATE admin.tiktok_returns
            SET
                return_status = $2,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            WHERE return_id = $1
            "#,
            return_id,
            new_status
        )
        .execute(self.pool)
        .await?;

        let updated = result.rows_affected() > 0;
        if updated {
            info!("TikTok return status updated");
        } else {
            debug!("TikTok return not found for status update");
        }

        Ok(updated)
    }
}

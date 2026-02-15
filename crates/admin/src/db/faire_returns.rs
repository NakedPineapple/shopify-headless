//! Faire returns repository.
//!
//! Caches Faire return/refund requests locally.
//! Returns are upserted by `faire_return_token`.

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

/// A cached Faire return/refund request.
#[derive(Debug, Clone)]
pub struct CachedFaireReturn {
    pub id: i32,
    pub faire_return_token: String,
    pub faire_order_token: Option<String>,
    pub return_status: String,
    pub return_reason: Option<String>,
    pub retailer_note: Option<String>,
    pub refund_amount: Option<String>,
    pub currency: Option<String>,
    pub decision_deadline: Option<DateTime<Utc>>,
    pub raw_json: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct ReturnRow {
    id: i32,
    faire_return_token: String,
    faire_order_token: Option<String>,
    return_status: String,
    return_reason: Option<String>,
    retailer_note: Option<String>,
    refund_amount: Option<String>,
    currency: Option<String>,
    decision_deadline: Option<DateTime<Utc>>,
    raw_json: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ReturnRow> for CachedFaireReturn {
    fn from(row: ReturnRow) -> Self {
        Self {
            id: row.id,
            faire_return_token: row.faire_return_token,
            faire_order_token: row.faire_order_token,
            return_status: row.return_status,
            return_reason: row.return_reason,
            retailer_note: row.retailer_note,
            refund_amount: row.refund_amount,
            currency: row.currency,
            decision_deadline: row.decision_deadline,
            raw_json: row.raw_json,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Parameters for upserting a Faire return.
pub struct UpsertFaireReturnParams<'a> {
    pub faire_return_token: &'a str,
    pub faire_order_token: Option<&'a str>,
    pub return_status: &'a str,
    pub return_reason: Option<&'a str>,
    pub retailer_note: Option<&'a str>,
    pub refund_amount: Option<&'a str>,
    pub currency: Option<&'a str>,
    pub decision_deadline: Option<DateTime<Utc>>,
    pub raw_json: Option<&'a serde_json::Value>,
}

impl std::fmt::Debug for UpsertFaireReturnParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpsertFaireReturnParams")
            .field("faire_return_token", &self.faire_return_token)
            .field("faire_order_token", &self.faire_order_token)
            .field("return_status", &self.return_status)
            .finish_non_exhaustive()
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for Faire return database operations.
pub struct FaireReturnRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> FaireReturnRepository<'a> {
    /// Create a new Faire return repository.
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
    ) -> Result<Vec<CachedFaireReturn>, RepositoryError> {
        debug!("Listing Faire returns");

        let rows = sqlx::query_as!(
            ReturnRow,
            r#"
            SELECT
                id, faire_return_token, faire_order_token,
                return_status, return_reason,
                retailer_note,
                refund_amount, currency,
                decision_deadline as "decision_deadline: DateTime<Utc>",
                raw_json,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.faire_returns
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

        Ok(rows.into_iter().map(CachedFaireReturn::from).collect())
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
            FROM admin.faire_returns
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
    pub async fn get_by_id(&self, id: i32) -> Result<Option<CachedFaireReturn>, RepositoryError> {
        debug!("Fetching Faire return by ID");

        let row = sqlx::query_as!(
            ReturnRow,
            r#"
            SELECT
                id, faire_return_token, faire_order_token,
                return_status, return_reason,
                retailer_note,
                refund_amount, currency,
                decision_deadline as "decision_deadline: DateTime<Utc>",
                raw_json,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.faire_returns
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(CachedFaireReturn::from))
    }

    /// Count returns grouped by status.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn count_by_status(&self) -> Result<Vec<(String, i64)>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT
                return_status as "status!",
                COUNT(*) as "count!"
            FROM admin.faire_returns
            GROUP BY return_status
            "#
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| (r.status, r.count)).collect())
    }

    /// Upsert a return (insert or update on conflict).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(return_token = %params.faire_return_token), level = "debug")]
    pub async fn upsert(
        &self,
        params: &UpsertFaireReturnParams<'_>,
    ) -> Result<i32, RepositoryError> {
        let decision_deadline = params.decision_deadline.map(to_time_offset);

        let row = sqlx::query_scalar!(
            r#"
            INSERT INTO admin.faire_returns (
                faire_return_token, faire_order_token, return_status,
                return_reason, retailer_note,
                refund_amount, currency,
                decision_deadline, raw_json
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (faire_return_token) DO UPDATE SET
                return_status = EXCLUDED.return_status,
                return_reason = EXCLUDED.return_reason,
                retailer_note = EXCLUDED.retailer_note,
                refund_amount = EXCLUDED.refund_amount,
                decision_deadline = EXCLUDED.decision_deadline,
                raw_json = EXCLUDED.raw_json,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            RETURNING id as "id!"
            "#,
            params.faire_return_token,
            params.faire_order_token,
            params.return_status,
            params.return_reason,
            params.retailer_note,
            params.refund_amount,
            params.currency,
            decision_deadline,
            params.raw_json
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
    #[instrument(skip(self), fields(faire_return_token = %faire_return_token, new_status = %status), level = "debug")]
    pub async fn update_status(
        &self,
        faire_return_token: &str,
        status: &str,
    ) -> Result<(), RepositoryError> {
        debug!("Updating Faire return status");

        let result = sqlx::query!(
            r#"
            UPDATE admin.faire_returns
            SET
                return_status = $2,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            WHERE faire_return_token = $1
            "#,
            faire_return_token,
            status
        )
        .execute(self.pool)
        .await?;

        if result.rows_affected() > 0 {
            info!("Faire return status updated");
        } else {
            debug!("Faire return not found for status update");
        }

        Ok(())
    }
}

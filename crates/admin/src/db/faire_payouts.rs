//! Faire payouts repository.
//!
//! Caches Faire payout reports and line items locally.
//! Payouts are upserted by `faire_payout_token`.

use chrono::{DateTime, Datelike, Timelike, Utc};
use sqlx::PgPool;
use tracing::{debug, instrument};

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

/// A cached Faire payout.
#[derive(Debug, Clone)]
pub struct CachedFairePayout {
    pub id: i32,
    pub faire_payout_token: String,
    pub payout_period_start: Option<DateTime<Utc>>,
    pub payout_period_end: Option<DateTime<Utc>>,
    pub total_revenue: Option<String>,
    pub total_refunds: Option<String>,
    pub total_commission: Option<String>,
    pub total_shipping_fees: Option<String>,
    pub net_payout: Option<String>,
    pub currency: Option<String>,
    pub payout_status: String,
    pub payout_date: Option<DateTime<Utc>>,
    pub raw_json: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct PayoutRow {
    id: i32,
    faire_payout_token: String,
    payout_period_start: Option<DateTime<Utc>>,
    payout_period_end: Option<DateTime<Utc>>,
    total_revenue: Option<String>,
    total_refunds: Option<String>,
    total_commission: Option<String>,
    total_shipping_fees: Option<String>,
    net_payout: Option<String>,
    currency: Option<String>,
    payout_status: String,
    payout_date: Option<DateTime<Utc>>,
    raw_json: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<PayoutRow> for CachedFairePayout {
    fn from(row: PayoutRow) -> Self {
        Self {
            id: row.id,
            faire_payout_token: row.faire_payout_token,
            payout_period_start: row.payout_period_start,
            payout_period_end: row.payout_period_end,
            total_revenue: row.total_revenue,
            total_refunds: row.total_refunds,
            total_commission: row.total_commission,
            total_shipping_fees: row.total_shipping_fees,
            net_payout: row.net_payout,
            currency: row.currency,
            payout_status: row.payout_status,
            payout_date: row.payout_date,
            raw_json: row.raw_json,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// A cached Faire payout line item.
#[derive(Debug, Clone)]
pub struct CachedFairePayoutLineItem {
    pub id: i32,
    pub faire_payout_token: String,
    pub faire_order_token: String,
    pub order_amount: Option<String>,
    pub refund_amount: Option<String>,
    pub commission_amount: Option<String>,
    pub shipping_fee: Option<String>,
    pub net_amount: Option<String>,
}

/// Parameters for upserting a Faire payout.
pub struct UpsertFairePayoutParams<'a> {
    pub faire_payout_token: &'a str,
    pub payout_period_start: Option<DateTime<Utc>>,
    pub payout_period_end: Option<DateTime<Utc>>,
    pub financials: UpsertPayoutFinancials<'a>,
    pub payout_status: &'a str,
    pub payout_date: Option<DateTime<Utc>>,
    pub raw_json: Option<&'a serde_json::Value>,
}

/// Financial fields for payout upsert (keeps param count under 7).
pub struct UpsertPayoutFinancials<'a> {
    pub total_revenue: Option<&'a str>,
    pub total_refunds: Option<&'a str>,
    pub total_commission: Option<&'a str>,
    pub total_shipping_fees: Option<&'a str>,
    pub net_payout: Option<&'a str>,
    pub currency: Option<&'a str>,
}

impl std::fmt::Debug for UpsertFairePayoutParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpsertFairePayoutParams")
            .field("faire_payout_token", &self.faire_payout_token)
            .field("payout_status", &self.payout_status)
            .finish_non_exhaustive()
    }
}

/// Parameters for upserting a Faire payout line item.
pub struct UpsertFairePayoutLineItemParams<'a> {
    pub faire_payout_token: &'a str,
    pub faire_order_token: &'a str,
    pub order_amount: Option<&'a str>,
    pub refund_amount: Option<&'a str>,
    pub commission_amount: Option<&'a str>,
    pub shipping_fee: Option<&'a str>,
    pub net_amount: Option<&'a str>,
}

impl std::fmt::Debug for UpsertFairePayoutLineItemParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpsertFairePayoutLineItemParams")
            .field("faire_payout_token", &self.faire_payout_token)
            .field("faire_order_token", &self.faire_order_token)
            .finish_non_exhaustive()
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for Faire payout database operations.
pub struct FairePayoutRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> FairePayoutRepository<'a> {
    /// Create a new Faire payout repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// List payouts with pagination and optional status filter.
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
    ) -> Result<Vec<CachedFairePayout>, RepositoryError> {
        debug!("Listing Faire payouts");

        let rows = sqlx::query_as!(
            PayoutRow,
            r#"
            SELECT
                id, faire_payout_token,
                payout_period_start as "payout_period_start: DateTime<Utc>",
                payout_period_end as "payout_period_end: DateTime<Utc>",
                total_revenue, total_refunds,
                total_commission, total_shipping_fees,
                net_payout, currency,
                payout_status,
                payout_date as "payout_date: DateTime<Utc>",
                raw_json,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.faire_payouts
            WHERE ($3::text IS NULL OR payout_status = $3)
            ORDER BY payout_period_end DESC NULLS LAST
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset,
            status_filter
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(CachedFairePayout::from).collect())
    }

    /// Count payouts with optional status filter.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn count(&self, status_filter: Option<&str>) -> Result<i64, RepositoryError> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM admin.faire_payouts
            WHERE ($1::text IS NULL OR payout_status = $1)
            "#,
            status_filter
        )
        .fetch_one(self.pool)
        .await?;

        Ok(count)
    }

    /// Get a single payout by database ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_by_id(&self, id: i32) -> Result<Option<CachedFairePayout>, RepositoryError> {
        debug!("Fetching Faire payout by ID");

        let row = sqlx::query_as!(
            PayoutRow,
            r#"
            SELECT
                id, faire_payout_token,
                payout_period_start as "payout_period_start: DateTime<Utc>",
                payout_period_end as "payout_period_end: DateTime<Utc>",
                total_revenue, total_refunds,
                total_commission, total_shipping_fees,
                net_payout, currency,
                payout_status,
                payout_date as "payout_date: DateTime<Utc>",
                raw_json,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.faire_payouts
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(CachedFairePayout::from))
    }

    /// Get line items for a payout.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_line_items(
        &self,
        faire_payout_token: &str,
    ) -> Result<Vec<CachedFairePayoutLineItem>, RepositoryError> {
        debug!("Fetching Faire payout line items");

        let items = sqlx::query_as!(
            CachedFairePayoutLineItem,
            r#"
            SELECT
                id, faire_payout_token, faire_order_token,
                order_amount, refund_amount, commission_amount,
                shipping_fee, net_amount
            FROM admin.faire_payout_line_items
            WHERE faire_payout_token = $1
            "#,
            faire_payout_token
        )
        .fetch_all(self.pool)
        .await?;

        Ok(items)
    }

    /// Upsert a payout (insert or update on conflict).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(payout_token = %params.faire_payout_token), level = "debug")]
    pub async fn upsert(
        &self,
        params: &UpsertFairePayoutParams<'_>,
    ) -> Result<i32, RepositoryError> {
        let period_start = params.payout_period_start.map(to_time_offset);
        let period_end = params.payout_period_end.map(to_time_offset);
        let payout_date = params.payout_date.map(to_time_offset);

        let row = sqlx::query_scalar!(
            r#"
            INSERT INTO admin.faire_payouts (
                faire_payout_token, payout_period_start, payout_period_end,
                total_revenue, total_refunds,
                total_commission, total_shipping_fees,
                net_payout, currency,
                payout_status, payout_date, raw_json
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (faire_payout_token) DO UPDATE SET
                payout_status = EXCLUDED.payout_status,
                total_revenue = EXCLUDED.total_revenue,
                total_refunds = EXCLUDED.total_refunds,
                total_commission = EXCLUDED.total_commission,
                total_shipping_fees = EXCLUDED.total_shipping_fees,
                net_payout = EXCLUDED.net_payout,
                payout_date = EXCLUDED.payout_date,
                raw_json = EXCLUDED.raw_json,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            RETURNING id as "id!"
            "#,
            params.faire_payout_token,
            period_start,
            period_end,
            params.financials.total_revenue,
            params.financials.total_refunds,
            params.financials.total_commission,
            params.financials.total_shipping_fees,
            params.financials.net_payout,
            params.financials.currency,
            params.payout_status,
            payout_date,
            params.raw_json
        )
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    /// Upsert a single payout line item.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), level = "debug")]
    pub async fn upsert_line_item(
        &self,
        params: &UpsertFairePayoutLineItemParams<'_>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r"
            INSERT INTO admin.faire_payout_line_items (
                faire_payout_token, faire_order_token,
                order_amount, refund_amount, commission_amount,
                shipping_fee, net_amount
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (faire_payout_token, faire_order_token) DO UPDATE SET
                order_amount = EXCLUDED.order_amount,
                refund_amount = EXCLUDED.refund_amount,
                commission_amount = EXCLUDED.commission_amount,
                shipping_fee = EXCLUDED.shipping_fee,
                net_amount = EXCLUDED.net_amount
            ",
            params.faire_payout_token,
            params.faire_order_token,
            params.order_amount,
            params.refund_amount,
            params.commission_amount,
            params.shipping_fee,
            params.net_amount
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Upsert payout line items in bulk.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, items), level = "debug")]
    pub async fn upsert_line_items(
        &self,
        items: &[UpsertFairePayoutLineItemParams<'_>],
    ) -> Result<(), RepositoryError> {
        for params in items {
            self.upsert_line_item(params).await?;
        }

        Ok(())
    }
}

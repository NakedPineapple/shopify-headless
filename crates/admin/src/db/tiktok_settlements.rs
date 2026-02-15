//! TikTok Shop settlement repository.
//!
//! Caches TikTok Shop settlement reports and line items locally.
//! Settlements are upserted by `settlement_id`.

use chrono::{DateTime, Datelike, Timelike, Utc};
use rust_decimal::Decimal;
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

/// A cached TikTok Shop settlement.
#[derive(Debug, Clone)]
pub struct CachedTikTokSettlement {
    pub id: i32,
    pub settlement_id: String,
    pub settlement_period_start: Option<DateTime<Utc>>,
    pub settlement_period_end: Option<DateTime<Utc>>,
    pub status: String,
    pub total_revenue: Option<String>,
    pub total_refunds: Option<String>,
    pub total_platform_fees: Option<String>,
    pub total_affiliate_commission: Option<String>,
    pub net_payout: Option<String>,
    pub currency: Option<String>,
    pub payout_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct SettlementRow {
    id: i32,
    settlement_id: String,
    settlement_period_start: Option<DateTime<Utc>>,
    settlement_period_end: Option<DateTime<Utc>>,
    status: String,
    total_revenue: Option<String>,
    total_refunds: Option<String>,
    total_platform_fees: Option<String>,
    total_affiliate_commission: Option<String>,
    net_payout: Option<String>,
    currency: Option<String>,
    payout_date: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<SettlementRow> for CachedTikTokSettlement {
    fn from(row: SettlementRow) -> Self {
        Self {
            id: row.id,
            settlement_id: row.settlement_id,
            settlement_period_start: row.settlement_period_start,
            settlement_period_end: row.settlement_period_end,
            status: row.status,
            total_revenue: row.total_revenue,
            total_refunds: row.total_refunds,
            total_platform_fees: row.total_platform_fees,
            total_affiliate_commission: row.total_affiliate_commission,
            net_payout: row.net_payout,
            currency: row.currency,
            payout_date: row.payout_date,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// A cached TikTok Shop settlement line item.
#[derive(Debug, Clone)]
pub struct CachedTikTokSettlementLineItem {
    pub id: i32,
    pub settlement_id: String,
    pub tiktok_order_id: String,
    pub order_amount: Option<String>,
    pub refund_amount: Option<String>,
    pub referral_fee: Option<String>,
    pub affiliate_commission: Option<String>,
    pub shipping_fee_subsidy: Option<String>,
    pub net_amount: Option<String>,
}

/// Parameters for upserting a TikTok settlement.
pub struct UpsertTikTokSettlementParams<'a> {
    pub settlement_id: &'a str,
    pub settlement_period_start: Option<DateTime<Utc>>,
    pub settlement_period_end: Option<DateTime<Utc>>,
    pub status: &'a str,
    pub total_revenue: Option<&'a str>,
    pub total_refunds: Option<&'a str>,
    pub financials: UpsertSettlementFinancials<'a>,
    pub payout_date: Option<DateTime<Utc>>,
}

/// Financial fields for settlement upsert (keeps param count under 7).
pub struct UpsertSettlementFinancials<'a> {
    pub total_platform_fees: Option<&'a str>,
    pub total_affiliate_commission: Option<&'a str>,
    pub net_payout: Option<&'a str>,
    pub currency: Option<&'a str>,
}

impl std::fmt::Debug for UpsertTikTokSettlementParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpsertTikTokSettlementParams")
            .field("settlement_id", &self.settlement_id)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

/// Parameters for upserting a TikTok settlement line item.
pub struct UpsertTikTokSettlementLineItemParams<'a> {
    pub settlement_id: &'a str,
    pub tiktok_order_id: &'a str,
    pub order_amount: Option<&'a str>,
    pub refund_amount: Option<&'a str>,
    pub referral_fee: Option<&'a str>,
    pub affiliate_commission: Option<&'a str>,
    pub shipping_fee_subsidy: Option<&'a str>,
    pub net_amount: Option<&'a str>,
}

impl std::fmt::Debug for UpsertTikTokSettlementLineItemParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpsertTikTokSettlementLineItemParams")
            .field("settlement_id", &self.settlement_id)
            .field("tiktok_order_id", &self.tiktok_order_id)
            .finish_non_exhaustive()
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for TikTok settlement database operations.
pub struct TikTokSettlementRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> TikTokSettlementRepository<'a> {
    /// Create a new TikTok settlement repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// List settlements with pagination and optional status filter.
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
    ) -> Result<Vec<CachedTikTokSettlement>, RepositoryError> {
        debug!("Listing TikTok settlements");

        let rows = sqlx::query_as!(
            SettlementRow,
            r#"
            SELECT
                id, settlement_id,
                settlement_period_start as "settlement_period_start: DateTime<Utc>",
                settlement_period_end as "settlement_period_end: DateTime<Utc>",
                status,
                total_revenue, total_refunds,
                total_platform_fees, total_affiliate_commission,
                net_payout, currency,
                payout_date as "payout_date: DateTime<Utc>",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.tiktok_settlements
            WHERE ($3::text IS NULL OR status = $3)
            ORDER BY settlement_period_end DESC NULLS LAST
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset,
            status_filter
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(CachedTikTokSettlement::from).collect())
    }

    /// Count settlements with optional status filter.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn count(&self, status_filter: Option<&str>) -> Result<i64, RepositoryError> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM admin.tiktok_settlements
            WHERE ($1::text IS NULL OR status = $1)
            "#,
            status_filter
        )
        .fetch_one(self.pool)
        .await?;

        Ok(count)
    }

    /// Get a single settlement by database ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_by_id(
        &self,
        id: i32,
    ) -> Result<Option<CachedTikTokSettlement>, RepositoryError> {
        debug!("Fetching TikTok settlement by ID");

        let row = sqlx::query_as!(
            SettlementRow,
            r#"
            SELECT
                id, settlement_id,
                settlement_period_start as "settlement_period_start: DateTime<Utc>",
                settlement_period_end as "settlement_period_end: DateTime<Utc>",
                status,
                total_revenue, total_refunds,
                total_platform_fees, total_affiliate_commission,
                net_payout, currency,
                payout_date as "payout_date: DateTime<Utc>",
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.tiktok_settlements
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(CachedTikTokSettlement::from))
    }

    /// Get line items for a settlement.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_line_items(
        &self,
        settlement_id: &str,
    ) -> Result<Vec<CachedTikTokSettlementLineItem>, RepositoryError> {
        debug!("Fetching TikTok settlement line items");

        let items = sqlx::query_as!(
            CachedTikTokSettlementLineItem,
            r#"
            SELECT
                id, settlement_id, tiktok_order_id,
                order_amount, refund_amount, referral_fee,
                affiliate_commission, shipping_fee_subsidy, net_amount
            FROM admin.tiktok_settlement_line_items
            WHERE settlement_id = $1
            "#,
            settlement_id
        )
        .fetch_all(self.pool)
        .await?;

        Ok(items)
    }

    /// Upsert a settlement (insert or update on conflict).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(settlement_id = %params.settlement_id), level = "debug")]
    pub async fn upsert(
        &self,
        params: &UpsertTikTokSettlementParams<'_>,
    ) -> Result<i32, RepositoryError> {
        let period_start = params.settlement_period_start.map(to_time_offset);
        let period_end = params.settlement_period_end.map(to_time_offset);
        let payout_date = params.payout_date.map(to_time_offset);

        let row = sqlx::query_scalar!(
            r#"
            INSERT INTO admin.tiktok_settlements (
                settlement_id, settlement_period_start, settlement_period_end,
                status, total_revenue, total_refunds,
                total_platform_fees, total_affiliate_commission,
                net_payout, currency, payout_date
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (settlement_id) DO UPDATE SET
                status = EXCLUDED.status,
                total_revenue = EXCLUDED.total_revenue,
                total_refunds = EXCLUDED.total_refunds,
                total_platform_fees = EXCLUDED.total_platform_fees,
                total_affiliate_commission = EXCLUDED.total_affiliate_commission,
                net_payout = EXCLUDED.net_payout,
                payout_date = EXCLUDED.payout_date,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            RETURNING id as "id!"
            "#,
            params.settlement_id,
            period_start,
            period_end,
            params.status,
            params.total_revenue,
            params.total_refunds,
            params.financials.total_platform_fees,
            params.financials.total_affiliate_commission,
            params.financials.net_payout,
            params.financials.currency,
            payout_date
        )
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    /// Upsert a settlement line item.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), level = "debug")]
    pub async fn upsert_line_item(
        &self,
        params: &UpsertTikTokSettlementLineItemParams<'_>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r"
            INSERT INTO admin.tiktok_settlement_line_items (
                settlement_id, tiktok_order_id,
                order_amount, refund_amount, referral_fee,
                affiliate_commission, shipping_fee_subsidy, net_amount
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (settlement_id, tiktok_order_id) DO UPDATE SET
                order_amount = EXCLUDED.order_amount,
                refund_amount = EXCLUDED.refund_amount,
                referral_fee = EXCLUDED.referral_fee,
                affiliate_commission = EXCLUDED.affiliate_commission,
                shipping_fee_subsidy = EXCLUDED.shipping_fee_subsidy,
                net_amount = EXCLUDED.net_amount
            ",
            params.settlement_id,
            params.tiktok_order_id,
            params.order_amount,
            params.refund_amount,
            params.referral_fee,
            params.affiliate_commission,
            params.shipping_fee_subsidy,
            params.net_amount
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }
}

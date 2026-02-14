//! Meta orders repository.
//!
//! Caches Facebook Shop and Instagram Shopping orders locally.
//! Orders are upserted by `facebook_order_id`.

use chrono::{DateTime, Datelike, NaiveDate, Timelike, Utc};
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

/// A cached Meta order.
#[derive(Debug, Clone)]
pub struct CachedMetaOrder {
    pub id: i32,
    pub facebook_order_id: String,
    pub shopify_order_id: Option<String>,
    pub created_time: Option<DateTime<Utc>>,
    pub last_updated_time: Option<DateTime<Utc>>,
    pub order_status: String,
    pub channel: String,
    pub buyer_name: Option<String>,
    pub buyer_email: Option<String>,
    pub ship_name: Option<String>,
    pub ship_street1: Option<String>,
    pub ship_street2: Option<String>,
    pub ship_city: Option<String>,
    pub ship_state: Option<String>,
    pub ship_postal_code: Option<String>,
    pub ship_country: Option<String>,
    pub estimated_payment_amount: Option<String>,
    pub estimated_payment_currency: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct OrderRow {
    id: i32,
    facebook_order_id: String,
    shopify_order_id: Option<String>,
    created_time: Option<DateTime<Utc>>,
    last_updated_time: Option<DateTime<Utc>>,
    order_status: String,
    channel: String,
    buyer_name: Option<String>,
    buyer_email: Option<String>,
    ship_name: Option<String>,
    ship_street1: Option<String>,
    ship_street2: Option<String>,
    ship_city: Option<String>,
    ship_state: Option<String>,
    ship_postal_code: Option<String>,
    ship_country: Option<String>,
    estimated_payment_amount: Option<String>,
    estimated_payment_currency: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<OrderRow> for CachedMetaOrder {
    fn from(row: OrderRow) -> Self {
        Self {
            id: row.id,
            facebook_order_id: row.facebook_order_id,
            shopify_order_id: row.shopify_order_id,
            created_time: row.created_time,
            last_updated_time: row.last_updated_time,
            order_status: row.order_status,
            channel: row.channel,
            buyer_name: row.buyer_name,
            buyer_email: row.buyer_email,
            ship_name: row.ship_name,
            ship_street1: row.ship_street1,
            ship_street2: row.ship_street2,
            ship_city: row.ship_city,
            ship_state: row.ship_state,
            ship_postal_code: row.ship_postal_code,
            ship_country: row.ship_country,
            estimated_payment_amount: row.estimated_payment_amount,
            estimated_payment_currency: row.estimated_payment_currency,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// A cached Meta order line item.
#[derive(Debug, Clone)]
pub struct CachedMetaOrderItem {
    pub id: i32,
    pub facebook_order_id: String,
    pub product_id: String,
    pub retailer_id: Option<String>,
    pub quantity: i32,
    pub price_per_unit: Option<String>,
    pub currency: Option<String>,
}

/// Parameters for upserting a Meta order.
pub struct UpsertMetaOrderParams<'a> {
    pub facebook_order_id: &'a str,
    pub created_time: Option<DateTime<Utc>>,
    pub last_updated_time: Option<DateTime<Utc>>,
    pub order_status: &'a str,
    pub channel: &'a str,
    pub buyer_name: Option<&'a str>,
    pub buyer_email: Option<&'a str>,
    pub ship_name: Option<&'a str>,
    pub ship_street1: Option<&'a str>,
    pub ship_street2: Option<&'a str>,
    pub ship_city: Option<&'a str>,
    pub ship_state: Option<&'a str>,
    pub ship_postal_code: Option<&'a str>,
    pub ship_country: Option<&'a str>,
    pub estimated_payment_amount: Option<&'a str>,
    pub estimated_payment_currency: Option<&'a str>,
    pub raw_json: Option<&'a serde_json::Value>,
}

impl std::fmt::Debug for UpsertMetaOrderParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpsertMetaOrderParams")
            .field("facebook_order_id", &self.facebook_order_id)
            .field("order_status", &self.order_status)
            .field("channel", &self.channel)
            .finish_non_exhaustive()
    }
}

/// Parameters for upserting a Meta order item.
#[derive(Debug)]
pub struct UpsertMetaOrderItemParams<'a> {
    pub facebook_order_id: &'a str,
    pub product_id: &'a str,
    pub retailer_id: Option<&'a str>,
    pub quantity: i32,
    pub price_per_unit: Option<&'a str>,
    pub currency: Option<&'a str>,
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for Meta order database operations.
pub struct MetaOrderRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> MetaOrderRepository<'a> {
    /// Create a new Meta order repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// List orders with pagination and optional filters.
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
        channel_filter: Option<&str>,
    ) -> Result<Vec<CachedMetaOrder>, RepositoryError> {
        debug!("Listing Meta orders");

        let rows = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                id, facebook_order_id, shopify_order_id,
                created_time as "created_time: DateTime<Utc>",
                last_updated_time as "last_updated_time: DateTime<Utc>",
                order_status, channel,
                buyer_name, buyer_email,
                ship_name, ship_street1, ship_street2,
                ship_city, ship_state, ship_postal_code, ship_country,
                estimated_payment_amount, estimated_payment_currency,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.meta_orders
            WHERE ($3::text IS NULL OR order_status = $3)
              AND ($4::text IS NULL OR channel = $4)
            ORDER BY created_time DESC NULLS LAST
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset,
            status_filter,
            channel_filter
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(CachedMetaOrder::from).collect())
    }

    /// Count orders with optional filters.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn count(
        &self,
        status_filter: Option<&str>,
        channel_filter: Option<&str>,
    ) -> Result<i64, RepositoryError> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM admin.meta_orders
            WHERE ($1::text IS NULL OR order_status = $1)
              AND ($2::text IS NULL OR channel = $2)
            "#,
            status_filter,
            channel_filter
        )
        .fetch_one(self.pool)
        .await?;

        Ok(count)
    }

    /// Get a single order by database ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_by_id(&self, id: i32) -> Result<Option<CachedMetaOrder>, RepositoryError> {
        debug!("Fetching Meta order by ID");

        let row = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                id, facebook_order_id, shopify_order_id,
                created_time as "created_time: DateTime<Utc>",
                last_updated_time as "last_updated_time: DateTime<Utc>",
                order_status, channel,
                buyer_name, buyer_email,
                ship_name, ship_street1, ship_street2,
                ship_city, ship_state, ship_postal_code, ship_country,
                estimated_payment_amount, estimated_payment_currency,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.meta_orders
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(CachedMetaOrder::from))
    }

    /// Get a single order by Facebook order ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_by_facebook_order_id(
        &self,
        facebook_order_id: &str,
    ) -> Result<Option<CachedMetaOrder>, RepositoryError> {
        debug!("Fetching Meta order by Facebook order ID");

        let row = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                id, facebook_order_id, shopify_order_id,
                created_time as "created_time: DateTime<Utc>",
                last_updated_time as "last_updated_time: DateTime<Utc>",
                order_status, channel,
                buyer_name, buyer_email,
                ship_name, ship_street1, ship_street2,
                ship_city, ship_state, ship_postal_code, ship_country,
                estimated_payment_amount, estimated_payment_currency,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.meta_orders
            WHERE facebook_order_id = $1
            "#,
            facebook_order_id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(CachedMetaOrder::from))
    }

    /// Get order items for an order.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_items(
        &self,
        facebook_order_id: &str,
    ) -> Result<Vec<CachedMetaOrderItem>, RepositoryError> {
        debug!("Fetching Meta order items");

        let items = sqlx::query_as!(
            CachedMetaOrderItem,
            r#"
            SELECT
                id, facebook_order_id, product_id, retailer_id,
                quantity, price_per_unit, currency
            FROM admin.meta_order_items
            WHERE facebook_order_id = $1
            "#,
            facebook_order_id
        )
        .fetch_all(self.pool)
        .await?;

        Ok(items)
    }

    /// Upsert an order (insert or update on conflict).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(order_id = %params.facebook_order_id), level = "debug")]
    pub async fn upsert(&self, params: &UpsertMetaOrderParams<'_>) -> Result<i32, RepositoryError> {
        let created_time = params.created_time.map(to_time_offset);
        let last_updated_time = params.last_updated_time.map(to_time_offset);

        let row = sqlx::query_scalar!(
            r#"
            INSERT INTO admin.meta_orders (
                facebook_order_id, created_time, last_updated_time,
                order_status, channel,
                buyer_name, buyer_email,
                ship_name, ship_street1, ship_street2,
                ship_city, ship_state, ship_postal_code, ship_country,
                estimated_payment_amount, estimated_payment_currency,
                raw_json
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            ON CONFLICT (facebook_order_id) DO UPDATE SET
                last_updated_time = EXCLUDED.last_updated_time,
                order_status = EXCLUDED.order_status,
                raw_json = EXCLUDED.raw_json,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            RETURNING id as "id!"
            "#,
            params.facebook_order_id,
            created_time,
            last_updated_time,
            params.order_status,
            params.channel,
            params.buyer_name,
            params.buyer_email,
            params.ship_name,
            params.ship_street1,
            params.ship_street2,
            params.ship_city,
            params.ship_state,
            params.ship_postal_code,
            params.ship_country,
            params.estimated_payment_amount,
            params.estimated_payment_currency,
            params.raw_json
        )
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    /// Upsert an order item.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), level = "debug")]
    pub async fn upsert_item(
        &self,
        params: &UpsertMetaOrderItemParams<'_>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r"
            INSERT INTO admin.meta_order_items (
                facebook_order_id, product_id, retailer_id,
                quantity, price_per_unit, currency
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (facebook_order_id, product_id) DO UPDATE SET
                quantity = EXCLUDED.quantity,
                price_per_unit = EXCLUDED.price_per_unit,
                currency = EXCLUDED.currency
            ",
            params.facebook_order_id,
            params.product_id,
            params.retailer_id,
            params.quantity,
            params.price_per_unit,
            params.currency
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    // =========================================================================
    // Analytics Queries
    // =========================================================================

    /// Revenue summary for a date range (excludes canceled orders).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn revenue_summary(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<MetaRevenueSummary, RepositoryError> {
        let start_t = to_time_date(start);
        let end_t = to_time_date(end);

        let row = sqlx::query!(
            r#"
            SELECT
                COALESCE(SUM(CAST(estimated_payment_amount AS DECIMAL)), 0) as "revenue!: rust_decimal::Decimal",
                COUNT(*) as "count!"
            FROM admin.meta_orders
            WHERE order_status != 'CANCELED'
              AND created_time >= $1::date
              AND created_time < ($2::date + INTERVAL '1 day')
            "#,
            start_t,
            end_t,
        )
        .fetch_one(self.pool)
        .await?;

        let revenue: f64 = row.revenue.to_string().parse().unwrap_or(0.0);
        let count = row.count;
        #[allow(
            clippy::cast_precision_loss,
            reason = "order count will never approach 2^52"
        )]
        let aov = if count > 0 {
            revenue / count as f64
        } else {
            0.0
        };

        Ok(MetaRevenueSummary {
            total_revenue: revenue,
            order_count: count,
            average_order_value: aov,
        })
    }

    /// Revenue summary split by channel (facebook vs instagram).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn channel_breakdown(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<ChannelBreakdown>, RepositoryError> {
        let start_t = to_time_date(start);
        let end_t = to_time_date(end);

        let rows = sqlx::query!(
            r#"
            SELECT
                channel as "channel!",
                COALESCE(SUM(CAST(estimated_payment_amount AS DECIMAL)), 0) as "revenue!: rust_decimal::Decimal",
                COUNT(*) as "count!"
            FROM admin.meta_orders
            WHERE order_status != 'CANCELED'
              AND created_time >= $1::date
              AND created_time < ($2::date + INTERVAL '1 day')
            GROUP BY channel
            "#,
            start_t,
            end_t,
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ChannelBreakdown {
                channel: r.channel,
                revenue: r.revenue.to_string().parse().unwrap_or(0.0),
                count: r.count,
            })
            .collect())
    }

    /// Daily revenue for a date range (for Chart.js trend line).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn daily_revenue(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<MetaDailyRevenue>, RepositoryError> {
        let start_t = to_time_date(start);
        let end_t = to_time_date(end);

        let rows = sqlx::query!(
            r#"
            SELECT
                created_time::date as "date!: NaiveDate",
                COALESCE(SUM(CAST(estimated_payment_amount AS DECIMAL)), 0) as "revenue!: rust_decimal::Decimal",
                COUNT(*) as "orders!"
            FROM admin.meta_orders
            WHERE order_status != 'CANCELED'
              AND created_time >= $1::date
              AND created_time < ($2::date + INTERVAL '1 day')
            GROUP BY created_time::date
            ORDER BY created_time::date
            "#,
            start_t,
            end_t,
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| MetaDailyRevenue {
                date: r.date,
                revenue: r.revenue.to_string().parse().unwrap_or(0.0),
                orders: r.orders,
            })
            .collect())
    }

    /// Count orders by status for a date range.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn status_breakdown(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<MetaStatusBreakdown>, RepositoryError> {
        let start_t = to_time_date(start);
        let end_t = to_time_date(end);

        let rows = sqlx::query!(
            r#"
            SELECT
                order_status as "status!",
                COUNT(*) as "count!"
            FROM admin.meta_orders
            WHERE created_time >= $1::date
              AND created_time < ($2::date + INTERVAL '1 day')
            GROUP BY order_status
            "#,
            start_t,
            end_t,
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| MetaStatusBreakdown {
                status: r.status,
                count: r.count,
            })
            .collect())
    }
}

// =============================================================================
// Analytics Types
// =============================================================================

/// Revenue summary for a date range.
#[derive(Debug, Clone)]
pub struct MetaRevenueSummary {
    pub total_revenue: f64,
    pub order_count: i64,
    pub average_order_value: f64,
}

/// Daily revenue data point for trend charts.
#[derive(Debug, Clone)]
pub struct MetaDailyRevenue {
    pub date: NaiveDate,
    pub revenue: f64,
    pub orders: i64,
}

/// Order count by status.
#[derive(Debug, Clone)]
pub struct MetaStatusBreakdown {
    pub status: String,
    pub count: i64,
}

/// Order count and revenue by channel (facebook vs instagram).
#[derive(Debug, Clone)]
pub struct ChannelBreakdown {
    pub channel: String,
    pub revenue: f64,
    pub count: i64,
}

// =============================================================================
// Sync State
// =============================================================================

/// Get the last sync timestamp for a sync type.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
pub async fn get_sync_watermark(
    pool: &PgPool,
    sync_type: &str,
) -> Result<Option<DateTime<Utc>>, RepositoryError> {
    let ts = sqlx::query_scalar!(
        r#"
        SELECT last_sync_at as "last_sync_at: DateTime<Utc>"
        FROM admin.meta_sync_state
        WHERE sync_type = $1
        "#,
        sync_type
    )
    .fetch_optional(pool)
    .await?;

    Ok(ts)
}

/// Update the sync watermark for a sync type.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
pub async fn upsert_sync_watermark(
    pool: &PgPool,
    sync_type: &str,
    last_sync_at: DateTime<Utc>,
    items_synced: i32,
    error: Option<&str>,
) -> Result<(), RepositoryError> {
    let ts = to_time_offset(last_sync_at);

    sqlx::query!(
        r"
        INSERT INTO admin.meta_sync_state (sync_type, last_sync_at, items_synced, error)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (sync_type) DO UPDATE SET
            last_sync_at = EXCLUDED.last_sync_at,
            items_synced = EXCLUDED.items_synced,
            error = EXCLUDED.error,
            updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
        ",
        sync_type,
        ts,
        items_synced,
        error
    )
    .execute(pool)
    .await?;

    Ok(())
}

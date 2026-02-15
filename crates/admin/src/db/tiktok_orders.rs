//! TikTok Shop orders repository.
//!
//! Caches TikTok Shop orders locally with TikTok-native fields
//! including affiliate tracking, FBT (Fulfilled by TikTok), and
//! creator commerce data. Orders are upserted by `tiktok_order_id`.

use chrono::{DateTime, Datelike, NaiveDate, Timelike, Utc};
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

/// A cached TikTok Shop order.
#[derive(Debug, Clone)]
pub struct CachedTikTokOrder {
    pub id: i32,
    pub tiktok_order_id: String,
    pub shopify_order_id: Option<String>,
    pub created_time: Option<DateTime<Utc>>,
    pub last_updated_time: Option<DateTime<Utc>>,
    pub order_status: String,
    pub buyer_name: Option<String>,
    pub buyer_email: Option<String>,
    pub buyer_phone: Option<String>,
    pub ship_name: Option<String>,
    pub ship_street1: Option<String>,
    pub ship_street2: Option<String>,
    pub ship_city: Option<String>,
    pub ship_state: Option<String>,
    pub ship_postal_code: Option<String>,
    pub ship_country: Option<String>,
    pub payment_amount: Option<String>,
    pub payment_currency: Option<String>,
    pub shipping_amount: Option<String>,
    pub platform_discount: Option<String>,
    pub source_type: Option<String>,
    pub creator_username: Option<String>,
    pub creator_id: Option<String>,
    pub is_affiliate_order: bool,
    pub commission_rate: Option<Decimal>,
    pub commission_amount: Option<String>,
    pub commission_status: Option<String>,
    pub is_fbt: bool,
    pub fbt_warehouse_id: Option<String>,
    pub shipping_provider_id: Option<String>,
    pub tracking_number: Option<String>,
    pub shipping_status: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct OrderRow {
    id: i32,
    tiktok_order_id: String,
    shopify_order_id: Option<String>,
    created_time: Option<DateTime<Utc>>,
    last_updated_time: Option<DateTime<Utc>>,
    order_status: String,
    buyer_name: Option<String>,
    buyer_email: Option<String>,
    buyer_phone: Option<String>,
    ship_name: Option<String>,
    ship_street1: Option<String>,
    ship_street2: Option<String>,
    ship_city: Option<String>,
    ship_state: Option<String>,
    ship_postal_code: Option<String>,
    ship_country: Option<String>,
    payment_amount: Option<String>,
    payment_currency: Option<String>,
    shipping_amount: Option<String>,
    platform_discount: Option<String>,
    source_type: Option<String>,
    creator_username: Option<String>,
    creator_id: Option<String>,
    is_affiliate_order: bool,
    commission_rate: Option<Decimal>,
    commission_amount: Option<String>,
    commission_status: Option<String>,
    is_fbt: bool,
    fbt_warehouse_id: Option<String>,
    shipping_provider_id: Option<String>,
    tracking_number: Option<String>,
    shipping_status: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<OrderRow> for CachedTikTokOrder {
    fn from(row: OrderRow) -> Self {
        Self {
            id: row.id,
            tiktok_order_id: row.tiktok_order_id,
            shopify_order_id: row.shopify_order_id,
            created_time: row.created_time,
            last_updated_time: row.last_updated_time,
            order_status: row.order_status,
            buyer_name: row.buyer_name,
            buyer_email: row.buyer_email,
            buyer_phone: row.buyer_phone,
            ship_name: row.ship_name,
            ship_street1: row.ship_street1,
            ship_street2: row.ship_street2,
            ship_city: row.ship_city,
            ship_state: row.ship_state,
            ship_postal_code: row.ship_postal_code,
            ship_country: row.ship_country,
            payment_amount: row.payment_amount,
            payment_currency: row.payment_currency,
            shipping_amount: row.shipping_amount,
            platform_discount: row.platform_discount,
            source_type: row.source_type,
            creator_username: row.creator_username,
            creator_id: row.creator_id,
            is_affiliate_order: row.is_affiliate_order,
            commission_rate: row.commission_rate,
            commission_amount: row.commission_amount,
            commission_status: row.commission_status,
            is_fbt: row.is_fbt,
            fbt_warehouse_id: row.fbt_warehouse_id,
            shipping_provider_id: row.shipping_provider_id,
            tracking_number: row.tracking_number,
            shipping_status: row.shipping_status,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// A cached TikTok Shop order line item.
#[derive(Debug, Clone)]
pub struct CachedTikTokOrderItem {
    pub id: i32,
    pub tiktok_order_id: String,
    pub product_id: String,
    pub sku_id: Option<String>,
    pub product_name: Option<String>,
    pub quantity: i32,
    pub sale_price: Option<String>,
    pub original_price: Option<String>,
    pub currency: Option<String>,
    pub seller_discount: Option<String>,
    pub platform_discount: Option<String>,
}

/// Parameters for upserting a TikTok order (core fields).
pub struct UpsertTikTokOrderParams<'a> {
    pub tiktok_order_id: &'a str,
    pub created_time: Option<DateTime<Utc>>,
    pub last_updated_time: Option<DateTime<Utc>>,
    pub order_status: &'a str,
    pub buyer_name: Option<&'a str>,
    pub buyer_email: Option<&'a str>,
    pub buyer_phone: Option<&'a str>,
    pub shipping: UpsertTikTokOrderShipping<'a>,
    pub payment: UpsertTikTokOrderPayment<'a>,
    pub affiliate: UpsertTikTokOrderAffiliate<'a>,
    pub fulfillment: UpsertTikTokOrderFulfillment<'a>,
    pub raw_json: Option<&'a serde_json::Value>,
}

/// Shipping-related fields for order upsert.
pub struct UpsertTikTokOrderShipping<'a> {
    pub name: Option<&'a str>,
    pub street1: Option<&'a str>,
    pub street2: Option<&'a str>,
    pub city: Option<&'a str>,
    pub state: Option<&'a str>,
    pub postal_code: Option<&'a str>,
    pub country: Option<&'a str>,
}

/// Payment-related fields for order upsert.
pub struct UpsertTikTokOrderPayment<'a> {
    pub payment_amount: Option<&'a str>,
    pub payment_currency: Option<&'a str>,
    pub shipping_amount: Option<&'a str>,
    pub platform_discount: Option<&'a str>,
}

/// Affiliate/creator-related fields for order upsert.
pub struct UpsertTikTokOrderAffiliate<'a> {
    pub source_type: Option<&'a str>,
    pub creator_username: Option<&'a str>,
    pub creator_id: Option<&'a str>,
    pub is_affiliate_order: bool,
    pub commission_rate: Option<Decimal>,
    pub commission_amount: Option<&'a str>,
    pub commission_status: Option<&'a str>,
}

/// Fulfillment-related fields for order upsert.
pub struct UpsertTikTokOrderFulfillment<'a> {
    pub is_fbt: bool,
    pub fbt_warehouse_id: Option<&'a str>,
    pub shipping_provider_id: Option<&'a str>,
    pub tracking_number: Option<&'a str>,
    pub shipping_status: Option<&'a str>,
}

impl std::fmt::Debug for UpsertTikTokOrderParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpsertTikTokOrderParams")
            .field("tiktok_order_id", &self.tiktok_order_id)
            .field("order_status", &self.order_status)
            .finish_non_exhaustive()
    }
}

/// Parameters for upserting a TikTok order item.
#[derive(Debug)]
pub struct UpsertTikTokOrderItemParams<'a> {
    pub tiktok_order_id: &'a str,
    pub product_id: &'a str,
    pub sku_id: Option<&'a str>,
    pub product_name: Option<&'a str>,
    pub quantity: i32,
    pub sale_price: Option<&'a str>,
    pub original_price: Option<&'a str>,
    pub currency: Option<&'a str>,
    pub seller_discount: Option<&'a str>,
    pub platform_discount: Option<&'a str>,
}

/// Filter parameters for listing TikTok orders.
#[derive(Debug, Default)]
pub struct TikTokOrderFilters<'a> {
    pub status: Option<&'a str>,
    pub source: Option<&'a str>,
    pub affiliate: Option<bool>,
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for TikTok order database operations.
pub struct TikTokOrderRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> TikTokOrderRepository<'a> {
    /// Create a new TikTok order repository.
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
        filters: &TikTokOrderFilters<'_>,
    ) -> Result<Vec<CachedTikTokOrder>, RepositoryError> {
        debug!("Listing TikTok orders");

        let rows = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                id, tiktok_order_id, shopify_order_id,
                created_time as "created_time: DateTime<Utc>",
                last_updated_time as "last_updated_time: DateTime<Utc>",
                order_status,
                buyer_name, buyer_email, buyer_phone,
                ship_name, ship_street1, ship_street2,
                ship_city, ship_state, ship_postal_code, ship_country,
                payment_amount, payment_currency,
                shipping_amount, platform_discount,
                source_type, creator_username, creator_id,
                is_affiliate_order,
                commission_rate as "commission_rate: Decimal",
                commission_amount, commission_status,
                is_fbt, fbt_warehouse_id,
                shipping_provider_id, tracking_number, shipping_status,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.tiktok_orders
            WHERE ($3::text IS NULL OR order_status = $3)
              AND ($4::text IS NULL OR source_type = $4)
              AND ($5::bool IS NULL OR is_affiliate_order = $5)
            ORDER BY created_time DESC NULLS LAST
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset,
            filters.status,
            filters.source,
            filters.affiliate
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(CachedTikTokOrder::from).collect())
    }

    /// Count orders with optional filters.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn count(&self, filters: &TikTokOrderFilters<'_>) -> Result<i64, RepositoryError> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM admin.tiktok_orders
            WHERE ($1::text IS NULL OR order_status = $1)
              AND ($2::text IS NULL OR source_type = $2)
              AND ($3::bool IS NULL OR is_affiliate_order = $3)
            "#,
            filters.status,
            filters.source,
            filters.affiliate
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
    pub async fn get_by_id(&self, id: i32) -> Result<Option<CachedTikTokOrder>, RepositoryError> {
        debug!("Fetching TikTok order by ID");

        let row = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                id, tiktok_order_id, shopify_order_id,
                created_time as "created_time: DateTime<Utc>",
                last_updated_time as "last_updated_time: DateTime<Utc>",
                order_status,
                buyer_name, buyer_email, buyer_phone,
                ship_name, ship_street1, ship_street2,
                ship_city, ship_state, ship_postal_code, ship_country,
                payment_amount, payment_currency,
                shipping_amount, platform_discount,
                source_type, creator_username, creator_id,
                is_affiliate_order,
                commission_rate as "commission_rate: Decimal",
                commission_amount, commission_status,
                is_fbt, fbt_warehouse_id,
                shipping_provider_id, tracking_number, shipping_status,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.tiktok_orders
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(CachedTikTokOrder::from))
    }

    /// Get a single order by TikTok order ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_by_tiktok_order_id(
        &self,
        tiktok_order_id: &str,
    ) -> Result<Option<CachedTikTokOrder>, RepositoryError> {
        debug!("Fetching TikTok order by TikTok order ID");

        let row = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                id, tiktok_order_id, shopify_order_id,
                created_time as "created_time: DateTime<Utc>",
                last_updated_time as "last_updated_time: DateTime<Utc>",
                order_status,
                buyer_name, buyer_email, buyer_phone,
                ship_name, ship_street1, ship_street2,
                ship_city, ship_state, ship_postal_code, ship_country,
                payment_amount, payment_currency,
                shipping_amount, platform_discount,
                source_type, creator_username, creator_id,
                is_affiliate_order,
                commission_rate as "commission_rate: Decimal",
                commission_amount, commission_status,
                is_fbt, fbt_warehouse_id,
                shipping_provider_id, tracking_number, shipping_status,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.tiktok_orders
            WHERE tiktok_order_id = $1
            "#,
            tiktok_order_id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(CachedTikTokOrder::from))
    }

    /// Get order items for an order.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_items(
        &self,
        tiktok_order_id: &str,
    ) -> Result<Vec<CachedTikTokOrderItem>, RepositoryError> {
        debug!("Fetching TikTok order items");

        let items = sqlx::query_as!(
            CachedTikTokOrderItem,
            r#"
            SELECT
                id, tiktok_order_id, product_id, sku_id,
                product_name, quantity, sale_price, original_price,
                currency, seller_discount, platform_discount
            FROM admin.tiktok_order_items
            WHERE tiktok_order_id = $1
            "#,
            tiktok_order_id
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
    #[instrument(skip(self, params), fields(order_id = %params.tiktok_order_id), level = "debug")]
    pub async fn upsert(
        &self,
        params: &UpsertTikTokOrderParams<'_>,
    ) -> Result<i32, RepositoryError> {
        let created_time = params.created_time.map(to_time_offset);
        let last_updated_time = params.last_updated_time.map(to_time_offset);
        let commission_rate_text = params.affiliate.commission_rate.map(|d| d.to_string());

        let row = sqlx::query_scalar!(
            r#"
            INSERT INTO admin.tiktok_orders (
                tiktok_order_id, created_time, last_updated_time,
                order_status,
                buyer_name, buyer_email, buyer_phone,
                ship_name, ship_street1, ship_street2,
                ship_city, ship_state, ship_postal_code, ship_country,
                payment_amount, payment_currency,
                shipping_amount, platform_discount,
                source_type, creator_username, creator_id,
                is_affiliate_order,
                commission_rate, commission_amount, commission_status,
                is_fbt, fbt_warehouse_id,
                shipping_provider_id, tracking_number, shipping_status,
                raw_json
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18,
                $19, $20, $21, $22,
                $23::text::numeric, $24, $25,
                $26, $27, $28, $29, $30, $31
            )
            ON CONFLICT (tiktok_order_id) DO UPDATE SET
                last_updated_time = EXCLUDED.last_updated_time,
                order_status = EXCLUDED.order_status,
                commission_rate = EXCLUDED.commission_rate,
                commission_amount = EXCLUDED.commission_amount,
                commission_status = EXCLUDED.commission_status,
                shipping_provider_id = EXCLUDED.shipping_provider_id,
                tracking_number = EXCLUDED.tracking_number,
                shipping_status = EXCLUDED.shipping_status,
                raw_json = EXCLUDED.raw_json,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            RETURNING id as "id!"
            "#,
            params.tiktok_order_id,
            created_time,
            last_updated_time,
            params.order_status,
            params.buyer_name,
            params.buyer_email,
            params.buyer_phone,
            params.shipping.name,
            params.shipping.street1,
            params.shipping.street2,
            params.shipping.city,
            params.shipping.state,
            params.shipping.postal_code,
            params.shipping.country,
            params.payment.payment_amount,
            params.payment.payment_currency,
            params.payment.shipping_amount,
            params.payment.platform_discount,
            params.affiliate.source_type,
            params.affiliate.creator_username,
            params.affiliate.creator_id,
            params.affiliate.is_affiliate_order,
            commission_rate_text,
            params.affiliate.commission_amount,
            params.affiliate.commission_status,
            params.fulfillment.is_fbt,
            params.fulfillment.fbt_warehouse_id,
            params.fulfillment.shipping_provider_id,
            params.fulfillment.tracking_number,
            params.fulfillment.shipping_status,
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
        params: &UpsertTikTokOrderItemParams<'_>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r"
            INSERT INTO admin.tiktok_order_items (
                tiktok_order_id, product_id, sku_id,
                product_name, quantity, sale_price, original_price,
                currency, seller_discount, platform_discount
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (tiktok_order_id, product_id) DO UPDATE SET
                sku_id = EXCLUDED.sku_id,
                product_name = EXCLUDED.product_name,
                quantity = EXCLUDED.quantity,
                sale_price = EXCLUDED.sale_price,
                original_price = EXCLUDED.original_price,
                currency = EXCLUDED.currency,
                seller_discount = EXCLUDED.seller_discount,
                platform_discount = EXCLUDED.platform_discount
            ",
            params.tiktok_order_id,
            params.product_id,
            params.sku_id,
            params.product_name,
            params.quantity,
            params.sale_price,
            params.original_price,
            params.currency,
            params.seller_discount,
            params.platform_discount
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
    /// Includes TikTok-specific metrics: total commission and platform fees.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn revenue_summary(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<TikTokRevenueSummary, RepositoryError> {
        let start_t = to_time_date(start);
        let end_t = to_time_date(end);

        let row = sqlx::query!(
            r#"
            SELECT
                COALESCE(SUM(CAST(payment_amount AS DECIMAL)), 0) as "revenue!: Decimal",
                COUNT(*) as "count!",
                COALESCE(SUM(CAST(commission_amount AS DECIMAL)), 0) as "commission!: Decimal",
                COALESCE(SUM(CAST(platform_discount AS DECIMAL)), 0) as "platform_fees!: Decimal"
            FROM admin.tiktok_orders
            WHERE order_status != 'CANCELLED'
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
        let total_commission: f64 = row.commission.to_string().parse().unwrap_or(0.0);
        let total_platform_fees: f64 = row.platform_fees.to_string().parse().unwrap_or(0.0);
        #[allow(
            clippy::cast_precision_loss,
            reason = "order count will never approach 2^52"
        )]
        let aov = if count > 0 {
            revenue / count as f64
        } else {
            0.0
        };

        Ok(TikTokRevenueSummary {
            total_revenue: revenue,
            order_count: count,
            average_order_value: aov,
            total_commission,
            total_platform_fees,
        })
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
    ) -> Result<Vec<TikTokDailyRevenue>, RepositoryError> {
        let start_t = to_time_date(start);
        let end_t = to_time_date(end);

        let rows = sqlx::query!(
            r#"
            SELECT
                created_time::date as "date!: NaiveDate",
                COALESCE(SUM(CAST(payment_amount AS DECIMAL)), 0) as "revenue!: Decimal",
                COUNT(*) as "orders!"
            FROM admin.tiktok_orders
            WHERE order_status != 'CANCELLED'
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
            .map(|r| TikTokDailyRevenue {
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
    ) -> Result<Vec<TikTokStatusBreakdown>, RepositoryError> {
        let start_t = to_time_date(start);
        let end_t = to_time_date(end);

        let rows = sqlx::query!(
            r#"
            SELECT
                order_status as "status!",
                COUNT(*) as "count!"
            FROM admin.tiktok_orders
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
            .map(|r| TikTokStatusBreakdown {
                status: r.status,
                count: r.count,
            })
            .collect())
    }

    /// Revenue breakdown by source type (organic, affiliate, ad, etc.).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn source_breakdown(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<SourceBreakdown>, RepositoryError> {
        let start_t = to_time_date(start);
        let end_t = to_time_date(end);

        let rows = sqlx::query!(
            r#"
            SELECT
                COALESCE(source_type, 'unknown') as "source_type!",
                COALESCE(SUM(CAST(payment_amount AS DECIMAL)), 0) as "revenue!: Decimal",
                COUNT(*) as "count!"
            FROM admin.tiktok_orders
            WHERE order_status != 'CANCELLED'
              AND created_time >= $1::date
              AND created_time < ($2::date + INTERVAL '1 day')
            GROUP BY source_type
            "#,
            start_t,
            end_t,
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| SourceBreakdown {
                source_type: r.source_type,
                revenue: r.revenue.to_string().parse().unwrap_or(0.0),
                count: r.count,
            })
            .collect())
    }

    /// Top creators by order count, revenue, and commission.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn creator_breakdown(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        limit: i64,
    ) -> Result<Vec<CreatorBreakdown>, RepositoryError> {
        let start_t = to_time_date(start);
        let end_t = to_time_date(end);

        let rows = sqlx::query!(
            r#"
            SELECT
                COALESCE(creator_username, 'unknown') as "creator_username!",
                COUNT(*) as "order_count!",
                COALESCE(SUM(CAST(payment_amount AS DECIMAL)), 0) as "revenue!: Decimal",
                COALESCE(SUM(CAST(commission_amount AS DECIMAL)), 0) as "commission!: Decimal"
            FROM admin.tiktok_orders
            WHERE is_affiliate_order = true
              AND creator_username IS NOT NULL
              AND created_time >= $1::date
              AND created_time < ($2::date + INTERVAL '1 day')
            GROUP BY creator_username
            ORDER BY "revenue!: Decimal" DESC
            LIMIT $3
            "#,
            start_t,
            end_t,
            limit,
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| CreatorBreakdown {
                creator_username: r.creator_username,
                order_count: r.order_count,
                revenue: r.revenue.to_string().parse().unwrap_or(0.0),
                commission: r.commission.to_string().parse().unwrap_or(0.0),
            })
            .collect())
    }

    /// Summary of affiliate order performance.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn affiliate_summary(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<AffiliateSummary, RepositoryError> {
        let start_t = to_time_date(start);
        let end_t = to_time_date(end);

        let row = sqlx::query!(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE is_affiliate_order = true) as "affiliate_orders!",
                COUNT(*) as "total_orders!",
                COALESCE(
                    SUM(CAST(commission_amount AS DECIMAL)) FILTER (WHERE is_affiliate_order = true),
                    0
                ) as "total_commission!: Decimal"
            FROM admin.tiktok_orders
            WHERE order_status != 'CANCELLED'
              AND created_time >= $1::date
              AND created_time < ($2::date + INTERVAL '1 day')
            "#,
            start_t,
            end_t,
        )
        .fetch_one(self.pool)
        .await?;

        let total_orders = row.affiliate_orders;
        let total_commission: f64 = row.total_commission.to_string().parse().unwrap_or(0.0);
        let all_orders = row.total_orders;
        #[allow(
            clippy::cast_precision_loss,
            reason = "order count will never approach 2^52"
        )]
        let conversion_rate = if all_orders > 0 {
            total_orders as f64 / all_orders as f64
        } else {
            0.0
        };

        Ok(AffiliateSummary {
            total_orders,
            total_commission,
            conversion_rate,
        })
    }
}

// =============================================================================
// Analytics Types
// =============================================================================

/// Revenue summary for a date range with TikTok-specific metrics.
#[derive(Debug, Clone)]
pub struct TikTokRevenueSummary {
    pub total_revenue: f64,
    pub order_count: i64,
    pub average_order_value: f64,
    pub total_commission: f64,
    pub total_platform_fees: f64,
}

/// Daily revenue data point for trend charts.
#[derive(Debug, Clone)]
pub struct TikTokDailyRevenue {
    pub date: NaiveDate,
    pub revenue: f64,
    pub orders: i64,
}

/// Order count by status.
#[derive(Debug, Clone)]
pub struct TikTokStatusBreakdown {
    pub status: String,
    pub count: i64,
}

/// Revenue and count by source type (organic, affiliate, ad, etc.).
#[derive(Debug, Clone)]
pub struct SourceBreakdown {
    pub source_type: String,
    pub revenue: f64,
    pub count: i64,
}

/// Top creator performance breakdown.
#[derive(Debug, Clone)]
pub struct CreatorBreakdown {
    pub creator_username: String,
    pub order_count: i64,
    pub revenue: f64,
    pub commission: f64,
}

/// Summary of affiliate order performance.
#[derive(Debug, Clone)]
pub struct AffiliateSummary {
    pub total_orders: i64,
    pub total_commission: f64,
    pub conversion_rate: f64,
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
        FROM admin.tiktok_sync_state
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
        INSERT INTO admin.tiktok_sync_state (sync_type, last_sync_at, items_synced, error)
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

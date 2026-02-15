//! Faire wholesale orders repository.
//!
//! Caches Faire orders locally with wholesale-native fields
//! including retailer info, payment terms, and commission data.
//! Orders are upserted by `faire_order_token`.

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

/// A cached Faire wholesale order.
#[derive(Debug, Clone)]
pub struct CachedFaireOrder {
    pub id: i32,
    pub faire_order_token: String,
    pub shopify_order_id: Option<String>,
    pub order_state: String,
    pub created_at_faire: Option<DateTime<Utc>>,
    pub retailer_id: Option<String>,
    pub retailer_name: Option<String>,
    pub retailer_email: Option<String>,
    pub retailer_phone: Option<String>,
    pub ship_name: Option<String>,
    pub ship_street1: Option<String>,
    pub ship_street2: Option<String>,
    pub ship_city: Option<String>,
    pub ship_state: Option<String>,
    pub ship_postal_code: Option<String>,
    pub ship_country: Option<String>,
    pub order_total: Option<String>,
    pub currency: Option<String>,
    pub shipping_cost: Option<String>,
    pub faire_commission: Option<String>,
    pub net_payout: Option<String>,
    pub is_first_order: bool,
    pub payment_terms: Option<String>,
    pub raw_json: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct OrderRow {
    id: i32,
    faire_order_token: String,
    shopify_order_id: Option<String>,
    order_state: String,
    created_at_faire: Option<DateTime<Utc>>,
    retailer_id: Option<String>,
    retailer_name: Option<String>,
    retailer_email: Option<String>,
    retailer_phone: Option<String>,
    ship_name: Option<String>,
    ship_street1: Option<String>,
    ship_street2: Option<String>,
    ship_city: Option<String>,
    ship_state: Option<String>,
    ship_postal_code: Option<String>,
    ship_country: Option<String>,
    order_total: Option<String>,
    currency: Option<String>,
    shipping_cost: Option<String>,
    faire_commission: Option<String>,
    net_payout: Option<String>,
    is_first_order: bool,
    payment_terms: Option<String>,
    raw_json: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<OrderRow> for CachedFaireOrder {
    fn from(row: OrderRow) -> Self {
        Self {
            id: row.id,
            faire_order_token: row.faire_order_token,
            shopify_order_id: row.shopify_order_id,
            order_state: row.order_state,
            created_at_faire: row.created_at_faire,
            retailer_id: row.retailer_id,
            retailer_name: row.retailer_name,
            retailer_email: row.retailer_email,
            retailer_phone: row.retailer_phone,
            ship_name: row.ship_name,
            ship_street1: row.ship_street1,
            ship_street2: row.ship_street2,
            ship_city: row.ship_city,
            ship_state: row.ship_state,
            ship_postal_code: row.ship_postal_code,
            ship_country: row.ship_country,
            order_total: row.order_total,
            currency: row.currency,
            shipping_cost: row.shipping_cost,
            faire_commission: row.faire_commission,
            net_payout: row.net_payout,
            is_first_order: row.is_first_order,
            payment_terms: row.payment_terms,
            raw_json: row.raw_json,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// A cached Faire order line item.
#[derive(Debug, Clone)]
pub struct CachedFaireOrderItem {
    pub id: i32,
    pub faire_order_token: String,
    pub product_token: String,
    pub product_option_token: Option<String>,
    pub product_name: Option<String>,
    pub quantity: i32,
    pub unit_price: Option<String>,
    pub total_price: Option<String>,
    pub sku: Option<String>,
}

/// Parameters for upserting a Faire order.
pub struct UpsertFaireOrderParams<'a> {
    pub faire_order_token: &'a str,
    pub shopify_order_id: Option<&'a str>,
    pub order_state: &'a str,
    pub created_at_faire: Option<DateTime<Utc>>,
    pub retailer: UpsertFaireOrderRetailer<'a>,
    pub shipping: UpsertFaireOrderShipping<'a>,
    pub financials: UpsertFaireOrderFinancials<'a>,
    pub raw_json: Option<&'a serde_json::Value>,
}

/// Retailer-related fields for order upsert.
pub struct UpsertFaireOrderRetailer<'a> {
    pub id: Option<&'a str>,
    pub name: Option<&'a str>,
    pub email: Option<&'a str>,
    pub phone: Option<&'a str>,
}

/// Shipping-related fields for order upsert.
pub struct UpsertFaireOrderShipping<'a> {
    pub name: Option<&'a str>,
    pub street1: Option<&'a str>,
    pub street2: Option<&'a str>,
    pub city: Option<&'a str>,
    pub state: Option<&'a str>,
    pub postal_code: Option<&'a str>,
    pub country: Option<&'a str>,
}

/// Financial fields for order upsert (keeps param count under 7).
pub struct UpsertFaireOrderFinancials<'a> {
    pub order_total: Option<&'a str>,
    pub currency: Option<&'a str>,
    pub shipping_cost: Option<&'a str>,
    pub faire_commission: Option<&'a str>,
    pub net_payout: Option<&'a str>,
    pub is_first_order: bool,
    pub payment_terms: Option<&'a str>,
}

impl std::fmt::Debug for UpsertFaireOrderParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpsertFaireOrderParams")
            .field("faire_order_token", &self.faire_order_token)
            .field("order_state", &self.order_state)
            .finish_non_exhaustive()
    }
}

/// Parameters for upserting a Faire order line item.
pub struct UpsertFaireOrderItemParams<'a> {
    pub faire_order_token: &'a str,
    pub product_token: &'a str,
    pub product_option_token: Option<&'a str>,
    pub product_name: Option<&'a str>,
    pub quantity: i32,
    pub unit_price: Option<&'a str>,
    pub total_price: Option<&'a str>,
    pub sku: Option<&'a str>,
}

impl std::fmt::Debug for UpsertFaireOrderItemParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpsertFaireOrderItemParams")
            .field("faire_order_token", &self.faire_order_token)
            .field("product_token", &self.product_token)
            .finish_non_exhaustive()
    }
}

// =============================================================================
// Analytics Types
// =============================================================================

/// Daily revenue data point for trend charts.
#[derive(Debug, Clone)]
pub struct FaireDailyRevenue {
    pub date: String,
    pub order_count: i64,
    pub total_revenue: String,
}

/// Revenue summary for a date range with Faire-specific metrics.
#[derive(Debug, Clone)]
pub struct FaireRevenueSummary {
    pub total_orders: i64,
    pub total_revenue: String,
    pub average_order_value: String,
    pub total_commission: String,
    pub net_payout: String,
}

/// Top retailer performance breakdown.
#[derive(Debug, Clone)]
pub struct RetailerBreakdown {
    pub retailer_name: String,
    pub order_count: i64,
    pub total_revenue: String,
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for Faire order database operations.
pub struct FaireOrderRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> FaireOrderRepository<'a> {
    /// Create a new Faire order repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// List orders with pagination and optional status filter.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn list(
        &self,
        limit: i64,
        offset: i64,
        status: Option<&str>,
    ) -> Result<Vec<CachedFaireOrder>, RepositoryError> {
        debug!("Listing Faire orders");

        let rows = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                id, faire_order_token, shopify_order_id,
                order_state,
                created_at_faire as "created_at_faire: DateTime<Utc>",
                retailer_id, retailer_name, retailer_email, retailer_phone,
                ship_name, ship_street1, ship_street2,
                ship_city, ship_state, ship_postal_code, ship_country,
                order_total, currency, shipping_cost,
                faire_commission, net_payout,
                is_first_order, payment_terms,
                raw_json,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.faire_orders
            WHERE ($1::text IS NULL OR order_state = $1)
            ORDER BY created_at_faire DESC NULLS LAST
            LIMIT $2 OFFSET $3
            "#,
            status,
            limit,
            offset
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(CachedFaireOrder::from).collect())
    }

    /// Count orders with optional status filter.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn count(&self, status: Option<&str>) -> Result<i64, RepositoryError> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM admin.faire_orders
            WHERE ($1::text IS NULL OR order_state = $1)
            "#,
            status
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
    pub async fn get_by_id(&self, id: i32) -> Result<Option<CachedFaireOrder>, RepositoryError> {
        debug!("Fetching Faire order by ID");

        let row = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                id, faire_order_token, shopify_order_id,
                order_state,
                created_at_faire as "created_at_faire: DateTime<Utc>",
                retailer_id, retailer_name, retailer_email, retailer_phone,
                ship_name, ship_street1, ship_street2,
                ship_city, ship_state, ship_postal_code, ship_country,
                order_total, currency, shipping_cost,
                faire_commission, net_payout,
                is_first_order, payment_terms,
                raw_json,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.faire_orders
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(CachedFaireOrder::from))
    }

    /// Get order items for a Faire order.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_items(
        &self,
        faire_order_token: &str,
    ) -> Result<Vec<CachedFaireOrderItem>, RepositoryError> {
        debug!("Fetching Faire order items");

        let items = sqlx::query_as!(
            CachedFaireOrderItem,
            r#"
            SELECT
                id, faire_order_token, product_token,
                product_option_token, product_name,
                quantity, unit_price, total_price, sku
            FROM admin.faire_order_items
            WHERE faire_order_token = $1
            "#,
            faire_order_token
        )
        .fetch_all(self.pool)
        .await?;

        Ok(items)
    }

    /// Count first orders (new retailer acquisitions).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn count_first_orders(&self) -> Result<i64, RepositoryError> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM admin.faire_orders
            WHERE is_first_order = true
            "#
        )
        .fetch_one(self.pool)
        .await?;

        Ok(count)
    }

    /// Count unique retailers across all orders.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn count_unique_retailers(&self) -> Result<i64, RepositoryError> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(DISTINCT retailer_id) as "count!"
            FROM admin.faire_orders
            WHERE retailer_id IS NOT NULL
            "#
        )
        .fetch_one(self.pool)
        .await?;

        Ok(count)
    }

    /// Upsert an order (insert or update on conflict).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(order_token = %params.faire_order_token), level = "debug")]
    pub async fn upsert(
        &self,
        params: &UpsertFaireOrderParams<'_>,
    ) -> Result<i32, RepositoryError> {
        let created_at_faire = params.created_at_faire.map(to_time_offset);

        let row = sqlx::query_scalar!(
            r#"
            INSERT INTO admin.faire_orders (
                faire_order_token, shopify_order_id, order_state,
                created_at_faire,
                retailer_id, retailer_name, retailer_email, retailer_phone,
                ship_name, ship_street1, ship_street2,
                ship_city, ship_state, ship_postal_code, ship_country,
                order_total, currency, shipping_cost,
                faire_commission, net_payout,
                is_first_order, payment_terms,
                raw_json
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18,
                $19, $20, $21, $22, $23
            )
            ON CONFLICT (faire_order_token) DO UPDATE SET
                order_state = EXCLUDED.order_state,
                faire_commission = EXCLUDED.faire_commission,
                net_payout = EXCLUDED.net_payout,
                payment_terms = EXCLUDED.payment_terms,
                raw_json = EXCLUDED.raw_json,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            RETURNING id as "id!"
            "#,
            params.faire_order_token,
            params.shopify_order_id,
            params.order_state,
            created_at_faire,
            params.retailer.id,
            params.retailer.name,
            params.retailer.email,
            params.retailer.phone,
            params.shipping.name,
            params.shipping.street1,
            params.shipping.street2,
            params.shipping.city,
            params.shipping.state,
            params.shipping.postal_code,
            params.shipping.country,
            params.financials.order_total,
            params.financials.currency,
            params.financials.shipping_cost,
            params.financials.faire_commission,
            params.financials.net_payout,
            params.financials.is_first_order,
            params.financials.payment_terms,
            params.raw_json
        )
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    /// Upsert an order line item (insert or update on conflict).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), fields(order_token = %params.faire_order_token), level = "debug")]
    pub async fn upsert_item(
        &self,
        params: &UpsertFaireOrderItemParams<'_>,
    ) -> Result<i32, RepositoryError> {
        let row = sqlx::query_scalar!(
            r#"
            INSERT INTO admin.faire_order_items (
                faire_order_token, product_token, product_option_token,
                product_name, quantity, unit_price, total_price, sku
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (faire_order_token, product_token) DO UPDATE SET
                product_option_token = EXCLUDED.product_option_token,
                product_name = EXCLUDED.product_name,
                quantity = EXCLUDED.quantity,
                unit_price = EXCLUDED.unit_price,
                total_price = EXCLUDED.total_price,
                sku = EXCLUDED.sku
            RETURNING id as "id!"
            "#,
            params.faire_order_token,
            params.product_token,
            params.product_option_token,
            params.product_name,
            params.quantity,
            params.unit_price,
            params.total_price,
            params.sku
        )
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    // =========================================================================
    // Analytics Queries
    // =========================================================================

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
    ) -> Result<Vec<FaireDailyRevenue>, RepositoryError> {
        let start_t = to_time_date(start);
        let end_t = to_time_date(end);

        let rows = sqlx::query!(
            r#"
            SELECT
                created_at_faire::date as "date!: NaiveDate",
                COUNT(*) as "order_count!",
                COALESCE(SUM(CAST(order_total AS DECIMAL)), 0) as "total_revenue!: Decimal"
            FROM admin.faire_orders
            WHERE order_state != 'CANCELED'
              AND created_at_faire >= $1::date
              AND created_at_faire < ($2::date + INTERVAL '1 day')
            GROUP BY created_at_faire::date
            ORDER BY created_at_faire::date
            "#,
            start_t,
            end_t,
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| FaireDailyRevenue {
                date: r.date.to_string(),
                order_count: r.order_count,
                total_revenue: r.total_revenue.to_string(),
            })
            .collect())
    }

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
    ) -> Result<FaireRevenueSummary, RepositoryError> {
        let start_t = to_time_date(start);
        let end_t = to_time_date(end);

        let row = sqlx::query!(
            r#"
            SELECT
                COUNT(*) as "total_orders!",
                COALESCE(SUM(CAST(order_total AS DECIMAL)), 0) as "total_revenue!: Decimal",
                COALESCE(SUM(CAST(faire_commission AS DECIMAL)), 0) as "total_commission!: Decimal",
                COALESCE(SUM(CAST(net_payout AS DECIMAL)), 0) as "net_payout!: Decimal"
            FROM admin.faire_orders
            WHERE order_state != 'CANCELED'
              AND created_at_faire >= $1::date
              AND created_at_faire < ($2::date + INTERVAL '1 day')
            "#,
            start_t,
            end_t,
        )
        .fetch_one(self.pool)
        .await?;

        let total_revenue: f64 = row.total_revenue.to_string().parse().unwrap_or(0.0);
        let count = row.total_orders;
        #[allow(
            clippy::cast_precision_loss,
            reason = "order count will never approach 2^52"
        )]
        let aov = if count > 0 {
            total_revenue / count as f64
        } else {
            0.0
        };

        Ok(FaireRevenueSummary {
            total_orders: count,
            total_revenue: row.total_revenue.to_string(),
            average_order_value: format!("{aov:.2}"),
            total_commission: row.total_commission.to_string(),
            net_payout: row.net_payout.to_string(),
        })
    }

    /// Top retailers by order count and revenue.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn retailer_breakdown(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<RetailerBreakdown>, RepositoryError> {
        let start_t = to_time_date(start);
        let end_t = to_time_date(end);

        let rows = sqlx::query!(
            r#"
            SELECT
                COALESCE(retailer_name, 'unknown') as "retailer_name!",
                COUNT(*) as "order_count!",
                COALESCE(SUM(CAST(order_total AS DECIMAL)), 0) as "total_revenue!: Decimal"
            FROM admin.faire_orders
            WHERE retailer_name IS NOT NULL
              AND order_state != 'CANCELED'
              AND created_at_faire >= $1::date
              AND created_at_faire < ($2::date + INTERVAL '1 day')
            GROUP BY retailer_name
            ORDER BY "order_count!" DESC
            LIMIT 10
            "#,
            start_t,
            end_t,
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| RetailerBreakdown {
                retailer_name: r.retailer_name,
                order_count: r.order_count,
                total_revenue: r.total_revenue.to_string(),
            })
            .collect())
    }
}

//! Amazon orders repository.
//!
//! Caches Amazon orders locally because the Orders API rate limit is
//! 1 request per 60 seconds. Orders are upserted by `amazon_order_id`.

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

/// A cached Amazon order.
#[derive(Debug, Clone)]
pub struct CachedAmazonOrder {
    pub id: i32,
    pub amazon_order_id: String,
    pub shopify_order_id: Option<String>,
    pub purchase_date: Option<DateTime<Utc>>,
    pub last_update_date: Option<DateTime<Utc>>,
    pub order_status: String,
    pub fulfillment_channel: Option<String>,
    pub sales_channel: Option<String>,
    pub order_type: Option<String>,
    pub order_total_amount: Option<String>,
    pub order_total_currency: Option<String>,
    pub number_of_items_shipped: Option<i32>,
    pub number_of_items_unshipped: Option<i32>,
    pub is_business_order: Option<bool>,
    pub is_prime: Option<bool>,
    pub marketplace_id: Option<String>,
    pub ship_name: Option<String>,
    pub ship_city: Option<String>,
    pub ship_state: Option<String>,
    pub ship_postal_code: Option<String>,
    pub ship_country: Option<String>,
    pub buyer_email: Option<String>,
    pub buyer_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Internal row type for `PostgreSQL` queries.
#[derive(Debug, sqlx::FromRow)]
struct OrderRow {
    id: i32,
    amazon_order_id: String,
    shopify_order_id: Option<String>,
    purchase_date: Option<DateTime<Utc>>,
    last_update_date: Option<DateTime<Utc>>,
    order_status: String,
    fulfillment_channel: Option<String>,
    sales_channel: Option<String>,
    order_type: Option<String>,
    order_total_amount: Option<String>,
    order_total_currency: Option<String>,
    number_of_items_shipped: Option<i32>,
    number_of_items_unshipped: Option<i32>,
    is_business_order: Option<bool>,
    is_prime: Option<bool>,
    marketplace_id: Option<String>,
    ship_name: Option<String>,
    ship_city: Option<String>,
    ship_state: Option<String>,
    ship_postal_code: Option<String>,
    ship_country: Option<String>,
    buyer_email: Option<String>,
    buyer_name: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<OrderRow> for CachedAmazonOrder {
    fn from(row: OrderRow) -> Self {
        Self {
            id: row.id,
            amazon_order_id: row.amazon_order_id,
            shopify_order_id: row.shopify_order_id,
            purchase_date: row.purchase_date,
            last_update_date: row.last_update_date,
            order_status: row.order_status,
            fulfillment_channel: row.fulfillment_channel,
            sales_channel: row.sales_channel,
            order_type: row.order_type,
            order_total_amount: row.order_total_amount,
            order_total_currency: row.order_total_currency,
            number_of_items_shipped: row.number_of_items_shipped,
            number_of_items_unshipped: row.number_of_items_unshipped,
            is_business_order: row.is_business_order,
            is_prime: row.is_prime,
            marketplace_id: row.marketplace_id,
            ship_name: row.ship_name,
            ship_city: row.ship_city,
            ship_state: row.ship_state,
            ship_postal_code: row.ship_postal_code,
            ship_country: row.ship_country,
            buyer_email: row.buyer_email,
            buyer_name: row.buyer_name,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// A cached Amazon order line item.
#[derive(Debug, Clone)]
pub struct CachedAmazonOrderItem {
    pub id: i32,
    pub amazon_order_id: String,
    pub order_item_id: String,
    pub asin: String,
    pub seller_sku: Option<String>,
    pub title: Option<String>,
    pub quantity_ordered: i32,
    pub quantity_shipped: Option<i32>,
    pub item_price_amount: Option<String>,
    pub item_price_currency: Option<String>,
    pub item_tax_amount: Option<String>,
    pub item_tax_currency: Option<String>,
}

/// Parameters for upserting an Amazon order item.
pub struct UpsertOrderItemParams<'a> {
    pub amazon_order_id: &'a str,
    pub order_item_id: &'a str,
    pub asin: &'a str,
    pub seller_sku: Option<&'a str>,
    pub title: Option<&'a str>,
    pub quantity_ordered: i32,
    pub quantity_shipped: Option<i32>,
    pub item_price_amount: Option<&'a str>,
    pub item_price_currency: Option<&'a str>,
    pub item_tax_amount: Option<&'a str>,
    pub item_tax_currency: Option<&'a str>,
}

/// Parameters for upserting an Amazon order.
pub struct UpsertOrderParams<'a> {
    pub amazon_order_id: &'a str,
    pub purchase_date: Option<DateTime<Utc>>,
    pub last_update_date: Option<DateTime<Utc>>,
    pub order_status: &'a str,
    pub fulfillment_channel: Option<&'a str>,
    pub sales_channel: Option<&'a str>,
    pub order_type: Option<&'a str>,
    pub order_total_amount: Option<&'a str>,
    pub order_total_currency: Option<&'a str>,
    pub number_of_items_shipped: Option<i32>,
    pub number_of_items_unshipped: Option<i32>,
    pub is_business_order: Option<bool>,
    pub is_prime: Option<bool>,
    pub marketplace_id: Option<&'a str>,
    pub ship_name: Option<&'a str>,
    pub ship_city: Option<&'a str>,
    pub ship_state: Option<&'a str>,
    pub ship_postal_code: Option<&'a str>,
    pub ship_country: Option<&'a str>,
    pub buyer_email: Option<&'a str>,
    pub buyer_name: Option<&'a str>,
    pub raw_json: Option<&'a serde_json::Value>,
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for Amazon order database operations.
pub struct AmazonOrderRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> AmazonOrderRepository<'a> {
    /// Create a new order repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// List orders with pagination, ordered by purchase date descending.
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
    ) -> Result<Vec<CachedAmazonOrder>, RepositoryError> {
        debug!("Listing Amazon orders");

        let rows = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                id, amazon_order_id, shopify_order_id,
                purchase_date as "purchase_date: DateTime<Utc>",
                last_update_date as "last_update_date: DateTime<Utc>",
                order_status, fulfillment_channel, sales_channel, order_type,
                order_total_amount, order_total_currency,
                number_of_items_shipped, number_of_items_unshipped,
                is_business_order, is_prime, marketplace_id,
                ship_name, ship_city, ship_state, ship_postal_code, ship_country,
                buyer_email, buyer_name,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.amazon_orders
            WHERE ($3::text IS NULL OR order_status = $3)
            ORDER BY purchase_date DESC NULLS LAST
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset,
            status_filter
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(CachedAmazonOrder::from).collect())
    }

    /// Count orders with optional status filter.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn count(&self, status_filter: Option<&str>) -> Result<i64, RepositoryError> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM admin.amazon_orders
            WHERE ($1::text IS NULL OR order_status = $1)
            "#,
            status_filter
        )
        .fetch_one(self.pool)
        .await?;

        Ok(count)
    }

    /// Get a single order by Amazon order ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get(
        &self,
        amazon_order_id: &str,
    ) -> Result<Option<CachedAmazonOrder>, RepositoryError> {
        debug!("Fetching Amazon order");

        let row = sqlx::query_as!(
            OrderRow,
            r#"
            SELECT
                id, amazon_order_id, shopify_order_id,
                purchase_date as "purchase_date: DateTime<Utc>",
                last_update_date as "last_update_date: DateTime<Utc>",
                order_status, fulfillment_channel, sales_channel, order_type,
                order_total_amount, order_total_currency,
                number_of_items_shipped, number_of_items_unshipped,
                is_business_order, is_prime, marketplace_id,
                ship_name, ship_city, ship_state, ship_postal_code, ship_country,
                buyer_email, buyer_name,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.amazon_orders
            WHERE amazon_order_id = $1
            "#,
            amazon_order_id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(CachedAmazonOrder::from))
    }

    /// Get order items for an order.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn get_items(
        &self,
        amazon_order_id: &str,
    ) -> Result<Vec<CachedAmazonOrderItem>, RepositoryError> {
        debug!("Fetching Amazon order items");

        let items = sqlx::query_as!(
            CachedAmazonOrderItem,
            r#"
            SELECT
                id, amazon_order_id, order_item_id, asin, seller_sku,
                title, quantity_ordered, quantity_shipped,
                item_price_amount, item_price_currency,
                item_tax_amount, item_tax_currency
            FROM admin.amazon_order_items
            WHERE amazon_order_id = $1
            "#,
            amazon_order_id
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
    #[instrument(skip(self, params), fields(order_id = %params.amazon_order_id), level = "debug")]
    pub async fn upsert(&self, params: &UpsertOrderParams<'_>) -> Result<i32, RepositoryError> {
        let purchase_date = params.purchase_date.map(to_time_offset);
        let last_update_date = params.last_update_date.map(to_time_offset);

        let row = sqlx::query_scalar!(
            r#"
            INSERT INTO admin.amazon_orders (
                amazon_order_id, purchase_date, last_update_date,
                order_status, fulfillment_channel, sales_channel, order_type,
                order_total_amount, order_total_currency,
                number_of_items_shipped, number_of_items_unshipped,
                is_business_order, is_prime, marketplace_id,
                ship_name, ship_city, ship_state, ship_postal_code, ship_country,
                buyer_email, buyer_name, raw_json
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                    $14, $15, $16, $17, $18, $19, $20, $21, $22)
            ON CONFLICT (amazon_order_id) DO UPDATE SET
                last_update_date = EXCLUDED.last_update_date,
                order_status = EXCLUDED.order_status,
                fulfillment_channel = EXCLUDED.fulfillment_channel,
                number_of_items_shipped = EXCLUDED.number_of_items_shipped,
                number_of_items_unshipped = EXCLUDED.number_of_items_unshipped,
                raw_json = EXCLUDED.raw_json,
                updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
            RETURNING id as "id!"
            "#,
            params.amazon_order_id,
            purchase_date,
            last_update_date,
            params.order_status,
            params.fulfillment_channel,
            params.sales_channel,
            params.order_type,
            params.order_total_amount,
            params.order_total_currency,
            params.number_of_items_shipped,
            params.number_of_items_unshipped,
            params.is_business_order,
            params.is_prime,
            params.marketplace_id,
            params.ship_name,
            params.ship_city,
            params.ship_state,
            params.ship_postal_code,
            params.ship_country,
            params.buyer_email,
            params.buyer_name,
            params.raw_json
        )
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    /// Upsert order items (insert or update on conflict).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, params), level = "debug")]
    pub async fn upsert_item(
        &self,
        params: &UpsertOrderItemParams<'_>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r"
            INSERT INTO admin.amazon_order_items (
                amazon_order_id, order_item_id, asin, seller_sku, title,
                quantity_ordered, quantity_shipped,
                item_price_amount, item_price_currency,
                item_tax_amount, item_tax_currency
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (amazon_order_id, order_item_id) DO UPDATE SET
                quantity_ordered = EXCLUDED.quantity_ordered,
                quantity_shipped = EXCLUDED.quantity_shipped,
                item_price_amount = EXCLUDED.item_price_amount,
                item_price_currency = EXCLUDED.item_price_currency,
                item_tax_amount = EXCLUDED.item_tax_amount,
                item_tax_currency = EXCLUDED.item_tax_currency
            ",
            params.amazon_order_id,
            params.order_item_id,
            params.asin,
            params.seller_sku,
            params.title,
            params.quantity_ordered,
            params.quantity_shipped,
            params.item_price_amount,
            params.item_price_currency,
            params.item_tax_amount,
            params.item_tax_currency
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }
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
        FROM admin.amazon_sync_state
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
        INSERT INTO admin.amazon_sync_state (sync_type, last_sync_at, items_synced, error)
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

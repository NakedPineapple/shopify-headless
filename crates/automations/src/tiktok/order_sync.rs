//! TikTok Shop order sync workflow.
//!
//! Periodically fetches orders from the TikTok Shop API and caches them in
//! the `admin.tiktok_orders` and `admin.tiktok_order_items` tables. Uses a
//! high-water-mark (last sync timestamp) stored in `admin.tiktok_sync_state`
//! to only fetch new orders.

use chrono::{DateTime, Utc};
use naked_pineapple_services::tiktok_shop::{TikTokOrder, TikTokShopClient};
use sqlx::PgPool;
use tracing::instrument;

use crate::db::to_time_offset;

const SYNC_TYPE: &str = "tiktok_orders";

/// Run the TikTok Shop order sync workflow.
///
/// Returns `true` on success, `false` on failure (for circuit breaker).
#[instrument(skip(pool, client))]
pub async fn poll_tiktok_orders(pool: &PgPool, client: &TikTokShopClient) -> bool {
    let watermark = match get_watermark(pool).await {
        Ok(wm) => wm,
        Err(e) => {
            tracing::error!(error = %e, "failed to read TikTok order sync watermark");
            return false;
        }
    };

    let since_ts = watermark.timestamp();
    tracing::info!(since = %watermark, "polling TikTok Shop orders");

    let orders = match client.get_all_orders_since(since_ts).await {
        Ok(orders) => orders,
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch TikTok Shop orders");
            update_watermark(pool, watermark, 0, Some(&e.to_string())).await;
            return false;
        }
    };

    if orders.is_empty() {
        tracing::debug!("no new TikTok Shop orders");
        return true;
    }

    let mut synced = 0;
    for order in &orders {
        if upsert_order(pool, order).await {
            synced += 1;
        }
    }

    tracing::info!(
        total = orders.len(),
        synced = synced,
        "TikTok Shop order sync complete"
    );

    let new_watermark = Utc::now();
    update_watermark(pool, new_watermark, synced, None).await;
    true
}

/// Get the high-water-mark for order sync (default: 30 days ago).
async fn get_watermark(pool: &PgPool) -> Result<DateTime<Utc>, sqlx::Error> {
    let row = sqlx::query_scalar!(
        r#"
        SELECT last_sync_at as "last_sync_at: DateTime<Utc>"
        FROM admin.tiktok_sync_state
        WHERE sync_type = $1
        "#,
        SYNC_TYPE
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.unwrap_or_else(|| Utc::now() - chrono::Duration::days(30)))
}

/// Update the sync watermark.
async fn update_watermark(
    pool: &PgPool,
    last_sync_at: DateTime<Utc>,
    items_synced: i32,
    error: Option<&str>,
) {
    let ts = to_time_offset(last_sync_at);
    let result = sqlx::query!(
        r"
        INSERT INTO admin.tiktok_sync_state (sync_type, last_sync_at, items_synced, error)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (sync_type) DO UPDATE SET
            last_sync_at = EXCLUDED.last_sync_at,
            items_synced = EXCLUDED.items_synced,
            error = EXCLUDED.error,
            updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
        ",
        SYNC_TYPE,
        ts,
        items_synced,
        error
    )
    .execute(pool)
    .await;

    if let Err(e) = result {
        tracing::error!(error = %e, "failed to update TikTok sync watermark");
    }
}

/// Upsert a single order from TikTok Shop into the local cache.
async fn upsert_order(pool: &PgPool, order: &TikTokOrder) -> bool {
    let Some(order_id) = order.id.as_deref() else {
        return false;
    };

    let raw_json = serde_json::to_value(order).ok();

    if !upsert_order_row(pool, order, order_id, raw_json.as_ref()).await {
        return false;
    }

    upsert_order_items(pool, order, order_id);
    true
}

/// Insert or update the order row in `admin.tiktok_orders`.
async fn upsert_order_row(
    pool: &PgPool,
    order: &TikTokOrder,
    order_id: &str,
    raw_json: Option<&serde_json::Value>,
) -> bool {
    let created_time = order
        .create_time
        .and_then(|ts| DateTime::from_timestamp(ts, 0))
        .map(to_time_offset);
    let last_updated_time = order
        .update_time
        .and_then(|ts| DateTime::from_timestamp(ts, 0))
        .map(to_time_offset);

    let order_status = order.status.as_deref().unwrap_or("UNKNOWN");
    let source_type = order.source_type.as_deref();
    let creator_username = order.creator.as_ref().and_then(|c| c.username.as_deref());
    let creator_id = order.creator.as_ref().and_then(|c| c.id.as_deref());
    let is_affiliate = order.is_affiliate_order.unwrap_or(false);
    let commission_rate = order.commission.as_ref().and_then(|c| c.rate.as_deref());
    let commission_amount = order.commission.as_ref().and_then(|c| c.amount.as_deref());
    let commission_status = order.commission.as_ref().and_then(|c| c.status.as_deref());
    let is_fbt = order
        .fulfillment_type
        .as_deref()
        .is_some_and(|f| f == "FBT");
    let fbt_warehouse_id = order.fbt_warehouse_id.as_deref();

    let result = upsert_order_fields(
        pool,
        order,
        &OrderCoreFields {
            order_id,
            created_time,
            last_updated_time,
            order_status,
            raw_json,
        },
        &OrderAffiliateFields {
            source_type,
            creator_username,
            creator_id,
            is_affiliate,
            commission_rate,
            commission_amount,
            commission_status,
        },
        &OrderFulfillmentFields {
            is_fbt,
            fbt_warehouse_id,
        },
    )
    .await;

    if let Err(e) = &result {
        tracing::warn!(
            order_id = %order_id,
            error = %e,
            "failed to upsert TikTok Shop order"
        );
    }

    result.is_ok()
}

/// Affiliate/creator fields for order upsert (avoids exceeding 7-arg limit).
struct OrderAffiliateFields<'a> {
    source_type: Option<&'a str>,
    creator_username: Option<&'a str>,
    creator_id: Option<&'a str>,
    is_affiliate: bool,
    commission_rate: Option<&'a str>,
    commission_amount: Option<&'a str>,
    commission_status: Option<&'a str>,
}

/// Fulfillment fields for order upsert.
struct OrderFulfillmentFields<'a> {
    is_fbt: bool,
    fbt_warehouse_id: Option<&'a str>,
}

/// Shipping/payment/address fields extracted from a TikTok order.
struct OrderAddressFields<'a> {
    buyer_name: Option<&'a str>,
    buyer_phone: Option<&'a str>,
    ship_street1: Option<&'a str>,
    ship_street2: Option<&'a str>,
    ship_city: Option<&'a str>,
    ship_state: Option<&'a str>,
    ship_postal_code: Option<&'a str>,
    ship_country: Option<&'a str>,
    payment_amount: Option<&'a str>,
    payment_currency: Option<&'a str>,
    shipping_amount: Option<&'a str>,
    platform_discount: Option<&'a str>,
    provider_id: Option<&'a str>,
    tracking: Option<&'a str>,
    ship_status: Option<&'a str>,
}

/// Extract address, payment, and shipping fields from an order.
fn extract_address_fields(order: &TikTokOrder) -> OrderAddressFields<'_> {
    let addr = order.recipient_address.as_ref();
    OrderAddressFields {
        buyer_name: addr.and_then(|a| a.name.as_deref()),
        buyer_phone: addr.and_then(|a| a.phone_number.as_deref()),
        ship_street1: addr.and_then(|a| a.address_line1.as_deref()),
        ship_street2: addr.and_then(|a| a.address_line2.as_deref()),
        ship_city: addr.and_then(|a| a.city.as_deref()),
        ship_state: addr.and_then(|a| a.state.as_deref()),
        ship_postal_code: addr.and_then(|a| a.zipcode.as_deref()),
        ship_country: addr.and_then(|a| a.country.as_deref()),
        payment_amount: order
            .payment
            .as_ref()
            .and_then(|p| p.total_amount.as_deref()),
        payment_currency: order.payment.as_ref().and_then(|p| p.currency.as_deref()),
        shipping_amount: order
            .payment
            .as_ref()
            .and_then(|p| p.shipping_fee.as_deref()),
        platform_discount: order
            .payment
            .as_ref()
            .and_then(|p| p.platform_discount.as_deref()),
        provider_id: order
            .shipping
            .as_ref()
            .and_then(|s| s.provider_id.as_deref()),
        tracking: order
            .shipping
            .as_ref()
            .and_then(|s| s.tracking_number.as_deref()),
        ship_status: order.shipping.as_ref().and_then(|s| s.status.as_deref()),
    }
}

/// Core order fields for the upsert query.
struct OrderCoreFields<'a> {
    order_id: &'a str,
    created_time: Option<time::OffsetDateTime>,
    last_updated_time: Option<time::OffsetDateTime>,
    order_status: &'a str,
    raw_json: Option<&'a serde_json::Value>,
}

/// Execute the INSERT ... ON CONFLICT query for an order row.
async fn upsert_order_fields(
    pool: &PgPool,
    order: &TikTokOrder,
    core: &OrderCoreFields<'_>,
    affiliate: &OrderAffiliateFields<'_>,
    fulfillment: &OrderFulfillmentFields<'_>,
) -> Result<(), sqlx::Error> {
    let f = extract_address_fields(order);

    sqlx::query!(
        r"
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
        ",
        core.order_id,
        core.created_time,
        core.last_updated_time,
        core.order_status,
        f.buyer_name,
        None::<&str>,
        f.buyer_phone,
        f.buyer_name,
        f.ship_street1,
        f.ship_street2,
        f.ship_city,
        f.ship_state,
        f.ship_postal_code,
        f.ship_country,
        f.payment_amount,
        f.payment_currency,
        f.shipping_amount,
        f.platform_discount,
        affiliate.source_type,
        affiliate.creator_username,
        affiliate.creator_id,
        affiliate.is_affiliate,
        affiliate.commission_rate,
        affiliate.commission_amount,
        affiliate.commission_status,
        fulfillment.is_fbt,
        fulfillment.fbt_warehouse_id,
        f.provider_id,
        f.tracking,
        f.ship_status,
        core.raw_json
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Upsert order items for a given order into `admin.tiktok_order_items`.
fn upsert_order_items(pool: &PgPool, order: &TikTokOrder, order_id: &str) {
    let Some(items) = &order.line_items else {
        return;
    };

    let pool = pool.clone();
    let order_id = order_id.to_string();
    let items = items.clone();

    tokio::spawn(async move {
        for item in &items {
            let Some(product_id) = item.product_id.as_deref() else {
                continue;
            };

            let result = sqlx::query!(
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
                order_id.as_str(),
                product_id,
                item.sku_id.as_deref(),
                item.product_name.as_deref(),
                item.quantity.unwrap_or(0),
                item.sale_price.as_deref(),
                item.original_price.as_deref(),
                item.currency.as_deref(),
                item.seller_discount.as_deref(),
                item.platform_discount.as_deref()
            )
            .execute(&pool)
            .await;

            if let Err(e) = result {
                tracing::warn!(
                    order_id = %order_id,
                    product_id = %product_id,
                    error = %e,
                    "failed to upsert TikTok Shop order item"
                );
            }
        }
    });
}

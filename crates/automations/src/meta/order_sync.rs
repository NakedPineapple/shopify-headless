//! Meta Commerce order sync workflow.
//!
//! Periodically fetches orders from the Meta Commerce API and caches them in
//! the `admin.meta_orders` and `admin.meta_order_items` tables. Uses a
//! high-water-mark (last sync timestamp) stored in `admin.meta_sync_state`
//! to only fetch new orders.

use chrono::{DateTime, Utc};
use naked_pineapple_services::meta_commerce::{FacebookOrder, MetaCommerceClient};
use sqlx::PgPool;
use tracing::instrument;

use crate::db::to_time_offset;

const SYNC_TYPE: &str = "meta_orders";

/// Run the Meta Commerce order sync workflow.
///
/// Returns `true` on success, `false` on failure (for circuit breaker).
#[instrument(skip(pool, meta))]
pub async fn poll_meta_orders(pool: &PgPool, meta: &MetaCommerceClient) -> bool {
    let watermark = match get_watermark(pool).await {
        Ok(wm) => wm,
        Err(e) => {
            tracing::error!(error = %e, "failed to read Meta order sync watermark");
            return false;
        }
    };

    let updated_after = watermark.to_rfc3339();
    tracing::info!(since = %updated_after, "polling Meta Commerce orders");

    let orders = match meta.get_all_orders_since(&updated_after).await {
        Ok(orders) => orders,
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch Meta Commerce orders");
            update_watermark(pool, watermark, 0, Some(&e.to_string())).await;
            return false;
        }
    };

    if orders.is_empty() {
        tracing::debug!("no new Meta Commerce orders");
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
        "Meta Commerce order sync complete"
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
        FROM admin.meta_sync_state
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
        INSERT INTO admin.meta_sync_state (sync_type, last_sync_at, items_synced, error)
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
        tracing::error!(error = %e, "failed to update Meta sync watermark");
    }
}

/// Upsert a single order from Meta Commerce into the local cache.
async fn upsert_order(pool: &PgPool, order: &FacebookOrder) -> bool {
    let created_time = order
        .created
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| to_time_offset(dt.with_timezone(&Utc)));

    let last_updated_time = order
        .last_updated
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| to_time_offset(dt.with_timezone(&Utc)));

    let order_status = order
        .order_status
        .as_ref()
        .and_then(|s| s.state.as_deref())
        .unwrap_or("CREATED");

    let channel = order.channel.as_deref().unwrap_or("facebook");
    let raw_json = serde_json::to_value(order).ok();

    if !upsert_order_row(
        pool,
        order,
        created_time,
        last_updated_time,
        order_status,
        channel,
        raw_json,
    )
    .await
    {
        return false;
    }

    upsert_order_items(pool, order);
    true
}

/// Insert or update the order row in `admin.meta_orders`.
async fn upsert_order_row(
    pool: &PgPool,
    order: &FacebookOrder,
    created_time: Option<time::OffsetDateTime>,
    last_updated_time: Option<time::OffsetDateTime>,
    order_status: &str,
    channel: &str,
    raw_json: Option<serde_json::Value>,
) -> bool {
    let result = sqlx::query!(
        r"
        INSERT INTO admin.meta_orders (
            facebook_order_id, created_time, last_updated_time,
            order_status, channel, buyer_name, buyer_email,
            ship_name, ship_street1, ship_street2,
            ship_city, ship_state, ship_postal_code, ship_country,
            estimated_payment_amount, estimated_payment_currency,
            raw_json
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
        ON CONFLICT (facebook_order_id) DO UPDATE SET
            last_updated_time = EXCLUDED.last_updated_time,
            order_status = EXCLUDED.order_status,
            buyer_name = EXCLUDED.buyer_name,
            buyer_email = EXCLUDED.buyer_email,
            raw_json = EXCLUDED.raw_json,
            updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
        ",
        order.id,
        created_time,
        last_updated_time,
        order_status,
        channel,
        order.buyer_details.as_ref().and_then(|b| b.name.as_deref()),
        order
            .buyer_details
            .as_ref()
            .and_then(|b| b.email.as_deref()),
        order
            .shipping_address
            .as_ref()
            .and_then(|a| a.name.as_deref()),
        order
            .shipping_address
            .as_ref()
            .and_then(|a| a.street1.as_deref()),
        order
            .shipping_address
            .as_ref()
            .and_then(|a| a.street2.as_deref()),
        order
            .shipping_address
            .as_ref()
            .and_then(|a| a.city.as_deref()),
        order
            .shipping_address
            .as_ref()
            .and_then(|a| a.state.as_deref()),
        order
            .shipping_address
            .as_ref()
            .and_then(|a| a.postal_code.as_deref()),
        order
            .shipping_address
            .as_ref()
            .and_then(|a| a.country.as_deref()),
        order
            .estimated_payment_details
            .as_ref()
            .and_then(|p| p.total_amount.as_ref())
            .and_then(|m| m.amount.as_deref()),
        order
            .estimated_payment_details
            .as_ref()
            .and_then(|p| p.total_amount.as_ref())
            .and_then(|m| m.currency.as_deref()),
        raw_json
    )
    .execute(pool)
    .await;

    if let Err(e) = &result {
        tracing::warn!(
            order_id = %order.id,
            error = %e,
            "failed to upsert Meta Commerce order"
        );
    }

    result.is_ok()
}

/// Upsert order items for a given order into `admin.meta_order_items`.
fn upsert_order_items(pool: &PgPool, order: &FacebookOrder) {
    let Some(items_data) = &order.items else {
        return;
    };

    let pool = pool.clone();
    let order_id = order.id.clone();
    let items = items_data.data.clone();

    tokio::spawn(async move {
        for item in &items {
            let result = sqlx::query!(
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
                order_id,
                item.product_id.as_deref(),
                item.retailer_id.as_deref(),
                item.quantity,
                item.price_per_unit
                    .as_ref()
                    .and_then(|m| m.amount.as_deref()),
                item.price_per_unit
                    .as_ref()
                    .and_then(|m| m.currency.as_deref()),
            )
            .execute(&pool)
            .await;

            if let Err(e) = result {
                tracing::warn!(
                    order_id = %order_id,
                    product_id = ?item.product_id,
                    error = %e,
                    "failed to upsert Meta Commerce order item"
                );
            }
        }
    });
}

//! Amazon order sync workflow.
//!
//! Periodically fetches orders from Amazon SP-API and caches them in
//! the `admin.amazon_orders` table. Uses a high-water-mark (last sync
//! timestamp) stored in `admin.amazon_sync_state` to only fetch new orders.

use chrono::{DateTime, Utc};
use naked_pineapple_services::amazon_sp::AmazonSpClient;
use sqlx::PgPool;
use tracing::instrument;

use crate::db::to_time_offset;

const SYNC_TYPE: &str = "amazon_orders";

/// Run the Amazon order sync workflow.
///
/// Returns `true` on success, `false` on failure (for circuit breaker).
#[instrument(skip(pool, amazon))]
pub async fn poll_amazon_orders(pool: &PgPool, amazon: &AmazonSpClient) -> bool {
    let watermark = match get_watermark(pool).await {
        Ok(wm) => wm,
        Err(e) => {
            tracing::error!(error = %e, "failed to read Amazon order sync watermark");
            return false;
        }
    };

    let created_after = watermark.to_rfc3339();
    tracing::info!(since = %created_after, "polling Amazon orders");

    let orders = match amazon.get_all_orders_since(&created_after).await {
        Ok(orders) => orders,
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch Amazon orders");
            update_watermark(pool, watermark, 0, Some(&e.to_string())).await;
            return false;
        }
    };

    if orders.is_empty() {
        tracing::debug!("no new Amazon orders");
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
        "Amazon order sync complete"
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
        FROM admin.amazon_sync_state
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
        INSERT INTO admin.amazon_sync_state (sync_type, last_sync_at, items_synced, error)
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
        tracing::error!(error = %e, "failed to update Amazon sync watermark");
    }
}

/// Upsert a single order from SP-API into the local cache.
async fn upsert_order(
    pool: &PgPool,
    order: &naked_pineapple_services::amazon_sp::AmazonOrder,
) -> bool {
    let purchase_date = order
        .purchase_date
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| to_time_offset(dt.with_timezone(&Utc)));

    let last_update_date = order
        .last_update_date
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| to_time_offset(dt.with_timezone(&Utc)));

    let order_status = order.order_status.as_deref().unwrap_or("Unknown");
    let raw_json = serde_json::to_value(order).ok();

    let result = sqlx::query!(
        r"
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
        ",
        order.amazon_order_id,
        purchase_date,
        last_update_date,
        order_status,
        order.fulfillment_channel.as_deref(),
        order.sales_channel.as_deref(),
        order.order_type.as_deref(),
        order.order_total.as_ref().and_then(|m| m.amount.as_deref()),
        order
            .order_total
            .as_ref()
            .and_then(|m| m.currency_code.as_deref()),
        order.number_of_items_shipped,
        order.number_of_items_unshipped,
        order.is_business_order,
        order.is_prime,
        order.marketplace_id.as_deref(),
        order
            .shipping_address
            .as_ref()
            .and_then(|a| a.name.as_deref()),
        order
            .shipping_address
            .as_ref()
            .and_then(|a| a.city.as_deref()),
        order
            .shipping_address
            .as_ref()
            .and_then(|a| a.state_or_region.as_deref()),
        order
            .shipping_address
            .as_ref()
            .and_then(|a| a.postal_code.as_deref()),
        order
            .shipping_address
            .as_ref()
            .and_then(|a| a.country_code.as_deref()),
        order
            .buyer_info
            .as_ref()
            .and_then(|b| b.buyer_email.as_deref()),
        order
            .buyer_info
            .as_ref()
            .and_then(|b| b.buyer_name.as_deref()),
        raw_json
    )
    .execute(pool)
    .await;

    if let Err(e) = &result {
        tracing::warn!(
            order_id = %order.amazon_order_id,
            error = %e,
            "failed to upsert Amazon order"
        );
    }

    result.is_ok()
}

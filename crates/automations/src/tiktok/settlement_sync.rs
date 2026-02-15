//! TikTok Shop settlement sync workflow.
//!
//! Periodically fetches settlements from the TikTok Shop Finance API and
//! caches them in `admin.tiktok_settlements` and
//! `admin.tiktok_settlement_line_items`. Uses a high-water-mark stored in
//! `admin.tiktok_sync_state` to track progress.

use chrono::{DateTime, Utc};
use naked_pineapple_services::tiktok_shop::{TikTokSettlement, TikTokShopClient};
use sqlx::PgPool;
use tracing::instrument;

use crate::db::to_time_offset;

const SYNC_TYPE: &str = "tiktok_settlements";

/// Run the TikTok Shop settlement sync workflow.
///
/// Returns `true` on success, `false` on failure (for circuit breaker).
#[instrument(skip(pool, client))]
pub async fn poll_tiktok_settlements(pool: &PgPool, client: &TikTokShopClient) -> bool {
    tracing::info!("polling TikTok Shop settlements");

    let settlements = match fetch_all_settlements(client).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch TikTok Shop settlements");
            update_watermark(pool, Utc::now(), 0, Some(&e.to_string())).await;
            return false;
        }
    };

    if settlements.is_empty() {
        tracing::debug!("no TikTok Shop settlements found");
        return true;
    }

    let mut synced = 0;
    for settlement in &settlements {
        if upsert_settlement(pool, client, settlement).await {
            synced += 1;
        }
    }

    tracing::info!(
        total = settlements.len(),
        synced = synced,
        "TikTok Shop settlement sync complete"
    );

    update_watermark(pool, Utc::now(), synced, None).await;
    true
}

/// Fetch all settlements following pagination.
async fn fetch_all_settlements(
    client: &TikTokShopClient,
) -> Result<Vec<TikTokSettlement>, Box<dyn std::error::Error + Send + Sync>> {
    let mut all = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let page = client.get_settlements(50, page_token.as_deref()).await?;

        if let Some(settlements) = page.settlements {
            all.extend(settlements);
        }

        match page.next_page_token.filter(|t| !t.is_empty()) {
            Some(token) => page_token = Some(token),
            None => break,
        }
    }

    Ok(all)
}

/// Upsert a single settlement and its line items.
async fn upsert_settlement(
    pool: &PgPool,
    client: &TikTokShopClient,
    settlement: &TikTokSettlement,
) -> bool {
    let Some(settlement_id) = settlement.id.as_deref() else {
        return false;
    };

    if !upsert_settlement_row(pool, settlement, settlement_id).await {
        return false;
    }

    // Fetch detailed line items for this settlement.
    upsert_settlement_line_items(pool, client, settlement_id).await
}

/// Insert or update the settlement row.
async fn upsert_settlement_row(
    pool: &PgPool,
    settlement: &TikTokSettlement,
    settlement_id: &str,
) -> bool {
    let period_start = settlement
        .period_start
        .and_then(|ts| DateTime::from_timestamp(ts, 0))
        .map(to_time_offset);
    let period_end = settlement
        .period_end
        .and_then(|ts| DateTime::from_timestamp(ts, 0))
        .map(to_time_offset);
    let payout_date = settlement
        .payout_date
        .and_then(|ts| DateTime::from_timestamp(ts, 0))
        .map(to_time_offset);
    let status = settlement.status.as_deref().unwrap_or("UNKNOWN");

    let result = sqlx::query!(
        r"
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
        ",
        settlement_id,
        period_start,
        period_end,
        status,
        settlement.total_revenue.as_deref(),
        settlement.total_refunds.as_deref(),
        settlement.total_platform_fees.as_deref(),
        settlement.total_affiliate_commission.as_deref(),
        settlement.net_payout.as_deref(),
        settlement.currency.as_deref(),
        payout_date
    )
    .execute(pool)
    .await;

    if let Err(e) = &result {
        tracing::warn!(
            settlement_id = %settlement_id,
            error = %e,
            "failed to upsert TikTok Shop settlement"
        );
    }

    result.is_ok()
}

/// Fetch and upsert line items for a settlement.
async fn upsert_settlement_line_items(
    pool: &PgPool,
    client: &TikTokShopClient,
    settlement_id: &str,
) -> bool {
    let details = match client.get_settlement_details(settlement_id).await {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(
                settlement_id = %settlement_id,
                error = %e,
                "failed to fetch TikTok Shop settlement details"
            );
            // Settlement row was already upserted; line items are best-effort.
            return true;
        }
    };

    let Some(items) = &details.line_items else {
        return true;
    };

    for item in items {
        let Some(order_id) = item.order_id.as_deref() else {
            continue;
        };

        let result = sqlx::query!(
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
            settlement_id,
            order_id,
            item.order_amount.as_deref(),
            item.refund_amount.as_deref(),
            item.referral_fee.as_deref(),
            item.affiliate_commission.as_deref(),
            item.shipping_fee_subsidy.as_deref(),
            item.net_amount.as_deref()
        )
        .execute(pool)
        .await;

        if let Err(e) = result {
            tracing::warn!(
                settlement_id = %settlement_id,
                order_id = %order_id,
                error = %e,
                "failed to upsert TikTok Shop settlement line item"
            );
        }
    }

    true
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
        tracing::error!(error = %e, "failed to update TikTok settlement sync watermark");
    }
}

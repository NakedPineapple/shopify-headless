//! TikTok Shop return sync workflow.
//!
//! Periodically fetches returns/refund requests from the TikTok Shop API and
//! caches them in `admin.tiktok_returns`. Uses a high-water-mark stored in
//! `admin.tiktok_sync_state` to track progress.

use chrono::{DateTime, Utc};
use naked_pineapple_services::tiktok_shop::{TikTokReturn, TikTokShopClient};
use sqlx::PgPool;
use tracing::instrument;

use crate::db::to_time_offset;

const SYNC_TYPE: &str = "tiktok_returns";

/// Run the TikTok Shop return sync workflow.
///
/// Returns `true` on success, `false` on failure (for circuit breaker).
#[instrument(skip(pool, client))]
pub async fn poll_tiktok_returns(pool: &PgPool, client: &TikTokShopClient) -> bool {
    tracing::info!("polling TikTok Shop returns");

    let returns = match fetch_all_returns(client).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch TikTok Shop returns");
            update_watermark(pool, Utc::now(), 0, Some(&e.to_string())).await;
            return false;
        }
    };

    if returns.is_empty() {
        tracing::debug!("no TikTok Shop returns found");
        return true;
    }

    let mut synced = 0;
    for ret in &returns {
        if upsert_return(pool, ret).await {
            synced += 1;
        }
    }

    tracing::info!(
        total = returns.len(),
        synced = synced,
        "TikTok Shop return sync complete"
    );

    update_watermark(pool, Utc::now(), synced, None).await;
    true
}

/// Fetch all returns following pagination.
async fn fetch_all_returns(
    client: &TikTokShopClient,
) -> Result<Vec<TikTokReturn>, Box<dyn std::error::Error + Send + Sync>> {
    let mut all = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let page = client.get_returns(50, page_token.as_deref()).await?;

        if let Some(returns) = page.returns {
            all.extend(returns);
        }

        match page.next_page_token.filter(|t| !t.is_empty()) {
            Some(token) => page_token = Some(token),
            None => break,
        }
    }

    Ok(all)
}

/// Upsert a single return into the local cache.
async fn upsert_return(pool: &PgPool, ret: &TikTokReturn) -> bool {
    let Some(return_id) = ret.id.as_deref() else {
        return false;
    };

    let decision_deadline = ret
        .decision_deadline
        .and_then(|ts| DateTime::from_timestamp(ts, 0))
        .map(to_time_offset);
    let return_status = ret.status.as_deref().unwrap_or("UNKNOWN");

    let result = sqlx::query!(
        r"
        INSERT INTO admin.tiktok_returns (
            return_id, tiktok_order_id, return_status,
            return_type, reason, buyer_note,
            refund_amount, currency,
            decision_deadline, return_tracking_number
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (return_id) DO UPDATE SET
            return_status = EXCLUDED.return_status,
            reason = EXCLUDED.reason,
            buyer_note = EXCLUDED.buyer_note,
            refund_amount = EXCLUDED.refund_amount,
            decision_deadline = EXCLUDED.decision_deadline,
            return_tracking_number = EXCLUDED.return_tracking_number,
            updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
        ",
        return_id,
        ret.order_id.as_deref(),
        return_status,
        ret.return_type.as_deref(),
        ret.reason.as_deref(),
        ret.buyer_note.as_deref(),
        ret.refund_amount.as_deref(),
        ret.currency.as_deref(),
        decision_deadline,
        ret.tracking_number.as_deref()
    )
    .execute(pool)
    .await;

    if let Err(e) = &result {
        tracing::warn!(
            return_id = %return_id,
            error = %e,
            "failed to upsert TikTok Shop return"
        );
    }

    result.is_ok()
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
        tracing::error!(error = %e, "failed to update TikTok return sync watermark");
    }
}

//! TikTok Shop performance metrics sync workflow.
//!
//! Fetches the current shop performance/health metrics and stores a daily
//! snapshot in `admin.tiktok_shop_performance`.

use naked_pineapple_services::tiktok_shop::TikTokShopClient;
use sqlx::PgPool;
use tracing::instrument;

/// Run the TikTok Shop performance metrics sync workflow.
///
/// Returns `true` on success, `false` on failure (for circuit breaker).
#[instrument(skip(pool, client))]
pub async fn poll_tiktok_performance(pool: &PgPool, client: &TikTokShopClient) -> bool {
    tracing::info!("polling TikTok Shop performance metrics");

    let metrics = match client.get_performance_metrics().await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch TikTok Shop performance metrics");
            return false;
        }
    };

    let today = chrono::Utc::now().date_naive();
    let snapshot_date = to_time_date(today);
    let overall_health = metrics.overall_health.as_deref().unwrap_or("UNKNOWN");
    let otd_rate = metrics.on_time_delivery_rate.map(|r| r.to_string());
    let ld_rate = metrics.late_dispatch_rate.map(|r| r.to_string());
    let sfc_rate = metrics.seller_fault_cancel_rate.map(|r| r.to_string());
    let cs_rate = metrics.customer_satisfaction_rate.map(|r| r.to_string());

    let result = sqlx::query!(
        r"
        INSERT INTO admin.tiktok_shop_performance (
            snapshot_date,
            on_time_delivery_rate,
            late_dispatch_rate,
            seller_fault_cancel_rate,
            customer_satisfaction_rate,
            overall_health
        )
        VALUES (
            $1,
            $2::text::numeric,
            $3::text::numeric,
            $4::text::numeric,
            $5::text::numeric,
            $6
        )
        ON CONFLICT (snapshot_date) DO UPDATE SET
            on_time_delivery_rate = EXCLUDED.on_time_delivery_rate,
            late_dispatch_rate = EXCLUDED.late_dispatch_rate,
            seller_fault_cancel_rate = EXCLUDED.seller_fault_cancel_rate,
            customer_satisfaction_rate = EXCLUDED.customer_satisfaction_rate,
            overall_health = EXCLUDED.overall_health,
            updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
        ",
        snapshot_date,
        otd_rate,
        ld_rate,
        sfc_rate,
        cs_rate,
        overall_health
    )
    .execute(pool)
    .await;

    if let Err(e) = result {
        tracing::error!(error = %e, "failed to upsert TikTok Shop performance snapshot");
        return false;
    }

    tracing::info!(
        date = %today,
        overall_health = %overall_health,
        "TikTok Shop performance snapshot saved"
    );

    true
}

/// Convert chrono `NaiveDate` to `time::Date` for `SQLx` bind params.
fn to_time_date(date: chrono::NaiveDate) -> time::Date {
    use chrono::Datelike;

    let month = u8::try_from(date.month()).expect("month in range 1-12");
    let day = u8::try_from(date.day()).expect("day in range 1-31");
    time::Date::from_calendar_date(
        date.year(),
        time::Month::try_from(month).expect("valid month"),
        day,
    )
    .expect("valid date")
}

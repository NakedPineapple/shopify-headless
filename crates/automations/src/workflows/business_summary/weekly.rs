//! Weekly business summary email workflow.

use naked_pineapple_services::email::EmailService;
use sqlx::PgPool;
use tracing::{error, info, instrument, warn};

use crate::db::automation_log;
use crate::shopify::ShopifyClient;

use super::data;
use super::templates;

/// Service references needed by the weekly summary workflow.
pub struct WeeklySummaryClients<'a> {
    pub pool: &'a PgPool,
    pub shopify: &'a ShopifyClient,
    pub email_service: &'a EmailService,
    pub recipients: &'a [String],
    pub low_stock_threshold: i32,
}

/// Run the weekly business summary email workflow.
#[instrument(skip(clients))]
pub async fn run(clients: &WeeklySummaryClients<'_>) {
    let start = std::time::Instant::now();

    let log_id = match automation_log::start_run(clients.pool, "weekly_summary").await {
        Ok(id) => id,
        Err(e) => {
            error!(error = %e, "failed to start weekly summary automation log");
            return;
        }
    };

    match send_weekly_summary(clients).await {
        Ok(sent_count) => {
            let duration = i64::try_from(start.elapsed().as_millis()).unwrap_or(0);
            if let Err(e) =
                automation_log::complete_run(clients.pool, log_id, 1, sent_count, None, duration)
                    .await
            {
                warn!(error = %e, "failed to complete weekly summary log");
            }
        }
        Err(msg) => {
            let duration = i64::try_from(start.elapsed().as_millis()).unwrap_or(0);
            if let Err(e) = automation_log::fail_run(clients.pool, log_id, &msg, duration).await {
                warn!(error = %e, "failed to record weekly summary failure");
            }
        }
    }
}

async fn send_weekly_summary(clients: &WeeklySummaryClients<'_>) -> Result<i32, String> {
    let data =
        data::collect_weekly_data(clients.pool, clients.shopify, clients.low_stock_threshold)
            .await?;

    let (html, text) = templates::render_weekly(&data);
    let subject = format!(
        "Weekly Business Report — {} – {}",
        data.week_start, data.week_end
    );
    let mut sent = 0i32;

    for recipient in clients.recipients {
        if let Err(e) = clients
            .email_service
            .send_multipart_email(recipient, &subject, &text, &html)
            .await
        {
            warn!(
                recipient = %recipient,
                error = %e,
                "failed to send weekly summary email"
            );
        } else {
            sent += 1;
        }
    }

    info!(recipients = sent, "weekly summary emails sent");
    Ok(sent)
}

//! Outbound email queue processor.
//!
//! Fetches queued emails from the database and sends them via the Microsoft 365
//! Graph API `sendMail` endpoint. Failed sends are retried on subsequent queue
//! processing cycles, up to `max_attempts`.

use sqlx::PgPool;
use tracing::{error, info, instrument, warn};

use crate::db::outbound_queue;
use crate::microsoft_graph::M365Client;

/// Maximum emails to process per tick.
const BATCH_SIZE: i64 = 20;

/// Process the outbound email queue: fetch ready emails and send them via M365.
#[instrument(skip(pool, m365))]
pub async fn process_queue(pool: &PgPool, m365: &M365Client, mailbox: &str) {
    let emails = match outbound_queue::fetch_ready(pool, BATCH_SIZE).await {
        Ok(emails) => emails,
        Err(e) => {
            error!(error = %e, "failed to fetch outbound queue");
            return;
        }
    };

    if emails.is_empty() {
        return;
    }

    info!(count = emails.len(), "processing outbound email queue");

    for email in &emails {
        send_single(pool, m365, mailbox, email).await;
    }
}

/// Send a single queued email via M365.
async fn send_single(
    pool: &PgPool,
    m365: &M365Client,
    mailbox: &str,
    email: &outbound_queue::QueuedEmail,
) {
    let attempt = email.attempts + 1;

    match m365
        .send_mail(
            mailbox,
            &email.to_address,
            email.to_name.as_deref(),
            &email.subject,
            &email.body_html,
        )
        .await
    {
        Ok(()) => {
            info!(
                email_id = email.id,
                email_type = %email.email_type,
                to = %email.to_address,
                "outbound email sent"
            );
            if let Err(e) = outbound_queue::mark_sent(pool, email.id).await {
                error!(email_id = email.id, error = %e, "failed to mark email as sent");
            }
        }
        Err(e) => {
            let exhausted = attempt >= email.max_attempts;
            if exhausted {
                error!(
                    email_id = email.id,
                    attempt,
                    max = email.max_attempts,
                    error = %e,
                    "outbound email permanently failed"
                );
            } else {
                warn!(
                    email_id = email.id,
                    attempt,
                    max = email.max_attempts,
                    error = %e,
                    "outbound email send failed, will retry"
                );
            }
            if let Err(db_err) = outbound_queue::mark_failed(pool, email.id, &e.to_string()).await {
                error!(
                    email_id = email.id,
                    error = %db_err,
                    "failed to record send failure"
                );
            }
        }
    }
}

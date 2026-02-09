//! Subscription lifecycle workflow.
//!
//! Runs on a schedule (default: daily) and performs:
//!
//! 1. **Renewal reminders**: Find active subscriptions renewing within N days
//!    and queue reminder emails.
//! 2. **Payment failure**: Detect failed billing attempts and queue notification
//!    emails.
//! 3. **Win-back**: Find recently cancelled subscriptions and queue win-back
//!    emails after a configurable delay.

use sqlx::PgPool;
use tracing::{debug, error, info, instrument, warn};

use crate::db::{automation_log, outbound_queue};
use crate::outbound::{self, EmailType};
use crate::shopify::ShopifyClient;
use crate::shopify::subscriptions::{self, SubscriptionContract};

/// Service references needed by the subscription lifecycle workflow.
pub struct SubscriptionClients<'a> {
    /// Database connection pool.
    pub pool: &'a PgPool,
    /// Shopify Admin API client.
    pub shopify: &'a ShopifyClient,
    /// Days before renewal to send a reminder.
    pub renewal_reminder_days: u64,
    /// Days after cancellation to send a win-back email.
    pub winback_delay_days: u64,
}

/// Run the complete subscription lifecycle workflow.
#[instrument(skip(clients))]
pub async fn run(clients: &SubscriptionClients<'_>) {
    let start = std::time::Instant::now();

    let log_id = match automation_log::start_run(clients.pool, "subscription_lifecycle").await {
        Ok(id) => id,
        Err(e) => {
            error!(error = %e, "failed to start automation log");
            return;
        }
    };

    match process_subscriptions(clients).await {
        Ok((processed, actioned)) => {
            let duration = i64::try_from(start.elapsed().as_millis()).unwrap_or(0);
            let metadata = serde_json::json!({
                "processed": processed,
                "actioned": actioned,
            });
            if let Err(e) = automation_log::complete_run(
                clients.pool,
                log_id,
                processed,
                actioned,
                Some(&metadata),
                duration,
            )
            .await
            {
                warn!(error = %e, "failed to complete automation log");
            }
        }
        Err(msg) => {
            let duration = i64::try_from(start.elapsed().as_millis()).unwrap_or(0);
            if let Err(e) = automation_log::fail_run(clients.pool, log_id, &msg, duration).await {
                warn!(error = %e, "failed to record automation failure");
            }
        }
    }
}

/// Process all subscription lifecycle events. Returns (processed, actioned).
async fn process_subscriptions(clients: &SubscriptionClients<'_>) -> Result<(i32, i32), String> {
    let contracts = subscriptions::fetch_active_contracts(clients.shopify)
        .await
        .map_err(|e| format!("failed to fetch active contracts: {e}"))?;

    let total = i32::try_from(contracts.len()).unwrap_or(0);
    let mut actioned = 0i32;

    // 1. Renewal reminders
    actioned += send_renewal_reminders(clients, &contracts).await;

    // 2. Payment failure notifications
    actioned += send_payment_failure_notifications(clients, &contracts).await;

    // 3. Win-back emails for cancelled subscriptions
    actioned += send_winback_emails(clients).await;

    if actioned > 0 {
        info!(
            total = total,
            actioned = actioned,
            "subscription lifecycle processing complete"
        );
    }

    Ok((total, actioned))
}

/// Send renewal reminder emails for subscriptions renewing within N days.
async fn send_renewal_reminders(
    clients: &SubscriptionClients<'_>,
    contracts: &[SubscriptionContract],
) -> i32 {
    let reminder_window = chrono::Utc::now()
        + chrono::Duration::days(i64::try_from(clients.renewal_reminder_days).unwrap_or(3));
    let now = chrono::Utc::now();

    let mut sent = 0i32;
    for contract in contracts {
        let Some(email) = &contract.customer_email else {
            continue;
        };
        let Some(next_billing) = &contract.next_billing_date else {
            continue;
        };

        let Ok(billing_date) = chrono::DateTime::parse_from_rfc3339(next_billing) else {
            continue;
        };
        let billing_utc = billing_date.with_timezone(&chrono::Utc);

        // Only send if renewal is within the reminder window and in the future
        if billing_utc > reminder_window || billing_utc <= now {
            continue;
        }

        // Deduplicate: check if we already queued a renewal reminder for this contract
        let reference_id = format!("renewal:{}", contract.id);
        match outbound_queue::exists(
            clients.pool,
            EmailType::SubscriptionRenewalReminder.as_str(),
            &reference_id,
        )
        .await
        {
            Ok(true) => continue,
            Ok(false) => {}
            Err(e) => {
                warn!(contract_id = %contract.id, error = %e, "failed to check renewal queue");
                continue;
            }
        }

        let customer_name = format_customer_name(contract);
        let data = outbound::SubscriptionRenewalData {
            customer_name,
            renewal_date: billing_utc.format("%B %d, %Y").to_string(),
            product_names: contract.line_items.clone(),
        };

        let to_name = format_customer_name_opt(contract);
        match outbound::enqueue_subscription_renewal_reminder(
            clients.pool,
            email,
            to_name.as_deref(),
            &reference_id,
            &data,
        )
        .await
        {
            Ok(id) => {
                info!(
                    email_id = id,
                    contract_id = %contract.id,
                    "queued subscription renewal reminder"
                );
                sent += 1;
            }
            Err(e) => {
                warn!(
                    contract_id = %contract.id,
                    error = %e,
                    "failed to queue renewal reminder"
                );
            }
        }
    }

    if sent > 0 {
        debug!(count = sent, "renewal reminders sent");
    }
    sent
}

/// Send payment failure notification emails.
async fn send_payment_failure_notifications(
    clients: &SubscriptionClients<'_>,
    contracts: &[SubscriptionContract],
) -> i32 {
    let mut sent = 0i32;
    for contract in contracts {
        let Some(email) = &contract.customer_email else {
            continue;
        };

        // Check for recent billing failures
        match subscriptions::has_recent_billing_failure(clients.shopify, &contract.id).await {
            Ok(Some(_)) => {} // Has a failure — proceed to notify
            Ok(None) => continue,
            Err(e) => {
                warn!(
                    contract_id = %contract.id,
                    error = %e,
                    "failed to check billing attempts"
                );
                continue;
            }
        }

        // Deduplicate
        let reference_id = format!("payment_failure:{}", contract.id);
        match automation_log::has_recent_alert(
            clients.pool,
            "subscription_payment_failure",
            &contract.id,
            48,
        )
        .await
        {
            Ok(true) => continue,
            Ok(false) => {}
            Err(e) => {
                warn!(
                    contract_id = %contract.id,
                    error = %e,
                    "failed to check recent payment failure alerts"
                );
            }
        }

        let customer_name = format_customer_name(contract);
        let data = outbound::PaymentFailureData {
            customer_name,
            product_names: contract.line_items.clone(),
        };

        let to_name = format_customer_name_opt(contract);
        match outbound::enqueue_payment_failure_notification(
            clients.pool,
            email,
            to_name.as_deref(),
            &reference_id,
            &data,
        )
        .await
        {
            Ok(id) => {
                info!(
                    email_id = id,
                    contract_id = %contract.id,
                    "queued payment failure notification"
                );
                log_payment_failure_alert(clients.pool, contract).await;
                sent += 1;
            }
            Err(e) => {
                warn!(
                    contract_id = %contract.id,
                    error = %e,
                    "failed to queue payment failure notification"
                );
            }
        }
    }

    if sent > 0 {
        debug!(count = sent, "payment failure notifications sent");
    }
    sent
}

/// Send win-back emails for recently cancelled subscriptions.
async fn send_winback_emails(clients: &SubscriptionClients<'_>) -> i32 {
    // Poll for cancellations within a broad window (subscriptions cancelled recently)
    let poll_window = clients.winback_delay_days * 24 * 60 + 1440; // delay + 1 day buffer
    let contracts =
        match subscriptions::fetch_cancelled_contracts(clients.shopify, poll_window).await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "failed to fetch cancelled contracts");
                return 0;
            }
        };

    let mut sent = 0i32;
    for contract in &contracts {
        let Some(email) = &contract.customer_email else {
            continue;
        };

        // Deduplicate
        let reference_id = format!("winback:{}", contract.id);
        match outbound_queue::exists(
            clients.pool,
            EmailType::SubscriptionWinBack.as_str(),
            &reference_id,
        )
        .await
        {
            Ok(true) => continue,
            Ok(false) => {}
            Err(e) => {
                warn!(contract_id = %contract.id, error = %e, "failed to check winback queue");
                continue;
            }
        }

        let customer_name = format_customer_name(contract);
        let data = outbound::WinBackData {
            customer_name,
            product_names: contract.line_items.clone(),
            store_url: "https://nakedpineapple.co".to_string(),
        };

        // Schedule for N days after cancellation (use now + winback_delay if no cancelled_at)
        let scheduled_for = chrono::Utc::now()
            + chrono::Duration::days(i64::try_from(clients.winback_delay_days).unwrap_or(14));

        let to_name = format_customer_name_opt(contract);
        match outbound::enqueue_subscription_winback(
            clients.pool,
            email,
            to_name.as_deref(),
            &reference_id,
            &data,
            scheduled_for,
        )
        .await
        {
            Ok(id) => {
                info!(
                    email_id = id,
                    contract_id = %contract.id,
                    scheduled = %scheduled_for,
                    "queued subscription win-back email"
                );
                sent += 1;
            }
            Err(e) => {
                warn!(
                    contract_id = %contract.id,
                    error = %e,
                    "failed to queue win-back email"
                );
            }
        }
    }

    if sent > 0 {
        debug!(count = sent, "win-back emails queued");
    }
    sent
}

/// Format a customer name from a subscription contract.
fn format_customer_name(contract: &SubscriptionContract) -> String {
    match (&contract.customer_first_name, &contract.customer_last_name) {
        (Some(first), Some(last)) => format!("{first} {last}"),
        (Some(first), None) => first.clone(),
        (None, Some(last)) => last.clone(),
        (None, None) => String::new(),
    }
}

/// Format customer name for the `to_name` field (None if empty).
fn format_customer_name_opt(contract: &SubscriptionContract) -> Option<String> {
    let name = format_customer_name(contract);
    if name.is_empty() { None } else { Some(name) }
}

/// Log a payment failure alert for deduplication.
async fn log_payment_failure_alert(pool: &PgPool, contract: &SubscriptionContract) {
    let metadata = serde_json::json!({
        "product_id": contract.id,
        "contract_id": contract.id,
    });

    let log_id = match automation_log::start_run(pool, "subscription_payment_failure").await {
        Ok(id) => id,
        Err(e) => {
            warn!(error = %e, "failed to log payment failure alert");
            return;
        }
    };

    if let Err(e) = automation_log::complete_run(pool, log_id, 1, 1, Some(&metadata), 0).await {
        warn!(error = %e, "failed to complete payment failure alert log");
    }
}

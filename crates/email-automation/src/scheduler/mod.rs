//! Task scheduler for the email automation service.
//!
//! Runs periodic tasks using `tokio::select!` over interval ticks:
//! - Email polling (default: every 2 minutes)
//! - Abandoned cart detection (default: every 15 minutes)
//! - Low stock monitoring (default: every hour)
//! - Outbound email queue processing (default: every 30 seconds)
//! - Customer segment sync (default: daily)

use std::time::Duration;

use tokio::sync::watch;
use tokio::time::interval;

use crate::state::AppState;

/// Scheduler that runs periodic automation tasks.
pub struct Scheduler {
    state: AppState,
}

impl Scheduler {
    /// Create a new scheduler.
    #[must_use]
    pub const fn new(state: AppState) -> Self {
        Self { state }
    }

    /// Run the scheduler loop until a shutdown signal is received.
    ///
    /// Each tick calls the corresponding workflow handler. Errors in individual
    /// handlers are logged but do not stop the scheduler.
    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        let config = &self.state.config().scheduler;

        let mut email_poll = interval(Duration::from_secs(config.email_poll_interval_secs));
        let mut cart_check = interval(Duration::from_secs(config.cart_check_interval_secs));
        let mut stock_check = interval(Duration::from_secs(config.stock_check_interval_secs));
        let mut outbound = interval(Duration::from_secs(config.outbound_interval_secs));
        let mut segment_sync = interval(Duration::from_secs(config.segment_sync_interval_secs));

        tracing::info!(
            email_poll_secs = config.email_poll_interval_secs,
            cart_check_secs = config.cart_check_interval_secs,
            stock_check_secs = config.stock_check_interval_secs,
            outbound_secs = config.outbound_interval_secs,
            segment_sync_secs = config.segment_sync_interval_secs,
            "scheduler started"
        );

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    tracing::info!("scheduler received shutdown signal");
                    break;
                }
                _ = email_poll.tick() => self.poll_emails().await,
                _ = cart_check.tick() => self.check_abandoned_carts().await,
                _ = stock_check.tick() => self.check_low_stock().await,
                _ = outbound.tick() => self.process_outbound_queue().await,
                _ = segment_sync.tick() => self.sync_customer_segments().await,
            }
        }

        tracing::info!("scheduler stopped");
    }

    /// Poll shared mailboxes for unread emails.
    async fn poll_emails(&self) {
        let mailboxes = self.state.m365().mailboxes().to_vec();
        for mailbox in &mailboxes {
            match self.state.m365().list_unread(mailbox).await {
                Ok(messages) => {
                    if !messages.is_empty() {
                        tracing::info!(
                            mailbox = %mailbox,
                            count = messages.len(),
                            "found unread messages"
                        );
                    }
                    // Phase 2 will add triage pipeline here
                }
                Err(e) => {
                    tracing::error!(
                        mailbox = %mailbox,
                        error = %e,
                        "failed to poll mailbox"
                    );
                }
            }
        }
    }

    /// Check for abandoned carts (placeholder for Phase 4).
    #[expect(
        clippy::unused_async,
        reason = "will be async in Phase 4; select! requires consistent arm types"
    )]
    async fn check_abandoned_carts(&self) {
        tracing::debug!("abandoned cart check: not yet implemented");
    }

    /// Check for low stock items (placeholder for Phase 5).
    #[expect(
        clippy::unused_async,
        reason = "will be async in Phase 5; select! requires consistent arm types"
    )]
    async fn check_low_stock(&self) {
        tracing::debug!("low stock check: not yet implemented");
    }

    /// Process the outbound email queue (placeholder for Phase 3).
    #[expect(
        clippy::unused_async,
        reason = "will be async in Phase 3; select! requires consistent arm types"
    )]
    async fn process_outbound_queue(&self) {
        tracing::debug!("outbound queue processing: not yet implemented");
    }

    /// Sync customer segments to Klaviyo (placeholder for Phase 5).
    #[expect(
        clippy::unused_async,
        reason = "will be async in Phase 5; select! requires consistent arm types"
    )]
    async fn sync_customer_segments(&self) {
        tracing::debug!("customer segment sync: not yet implemented");
    }
}

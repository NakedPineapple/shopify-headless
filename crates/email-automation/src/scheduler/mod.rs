//! Task scheduler for the email automation service.
//!
//! Runs periodic tasks using `tokio::select!` over interval ticks:
//! - Email polling (default: every 2 minutes)
//! - Shopify order/fulfillment polling (default: every 5 minutes)
//! - Abandoned cart detection (default: every 15 minutes)
//! - Low stock monitoring (default: every hour)
//! - Outbound email queue processing (default: every 30 seconds)
//! - Customer segment sync (default: daily)

use std::time::Duration;

use tokio::sync::watch;
use tokio::time::interval;

use crate::outbound;
use crate::state::AppState;
use crate::triage;
use crate::workflows;

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
        let mut order_poll = interval(Duration::from_secs(config.order_poll_interval_secs));
        let mut cart_check = interval(Duration::from_secs(config.cart_check_interval_secs));
        let mut stock_check = interval(Duration::from_secs(config.stock_check_interval_secs));
        let mut outbound = interval(Duration::from_secs(config.outbound_interval_secs));
        let mut segment_sync = interval(Duration::from_secs(config.segment_sync_interval_secs));

        tracing::info!(
            email_poll_secs = config.email_poll_interval_secs,
            order_poll_secs = config.order_poll_interval_secs,
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
                _ = order_poll.tick() => self.poll_shopify_events().await,
                _ = cart_check.tick() => self.check_abandoned_carts().await,
                _ = stock_check.tick() => self.check_low_stock().await,
                _ = outbound.tick() => self.process_outbound_queue().await,
                _ = segment_sync.tick() => self.sync_customer_segments().await,
            }
        }

        tracing::info!("scheduler stopped");
    }

    /// Poll shared mailboxes for unread emails and run the triage pipeline.
    async fn poll_emails(&self) {
        let mailboxes = self.state.m365().mailboxes().to_vec();
        for mailbox in &mailboxes {
            match self.state.m365().list_unread(mailbox).await {
                Ok(messages) => {
                    if messages.is_empty() {
                        continue;
                    }
                    tracing::info!(
                        mailbox = %mailbox,
                        count = messages.len(),
                        "found unread messages"
                    );

                    let clients = triage::TriageClients {
                        pool: self.state.pool(),
                        m365: self.state.m365(),
                        claude: self.state.claude(),
                        slack: self.state.slack(),
                        klaviyo: self.state.klaviyo(),
                        shopify: self.state.shopify(),
                    };

                    triage::process_messages(&clients, mailbox, messages).await;
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

    /// Poll Shopify for recent orders/fulfillments and queue outbound emails.
    async fn poll_shopify_events(&self) {
        let Some(shopify) = self.state.shopify() else {
            return;
        };

        let poll_minutes = self.state.config().scheduler.order_poll_interval_secs / 60 + 2;
        let review_delay = self.state.config().scheduler.review_request_delay_days;
        let pool = self.state.pool();

        outbound::poller::poll_new_orders(pool, shopify, poll_minutes).await;
        outbound::poller::poll_fulfillments(pool, shopify, poll_minutes).await;
        outbound::poller::poll_deliveries(pool, shopify, poll_minutes, review_delay).await;
    }

    /// Detect abandoned carts, trigger Klaviyo recovery flows, and check for recoveries.
    async fn check_abandoned_carts(&self) {
        let (Some(shopify), Some(klaviyo)) = (self.state.shopify(), self.state.klaviyo()) else {
            return;
        };

        let config = &self.state.config().scheduler;
        let poll_window = config.cart_check_interval_secs / 60 + 5;

        let clients = workflows::abandoned_cart::AbandonedCartClients {
            pool: self.state.pool(),
            shopify,
            klaviyo,
            abandon_delay_minutes: config.cart_abandon_delay_minutes,
            poll_window_minutes: poll_window,
        };

        workflows::abandoned_cart::run(&clients).await;
    }

    /// Check for low stock items and send Slack/email alerts.
    async fn check_low_stock(&self) {
        let (Some(shopify), Some(slack)) = (self.state.shopify(), self.state.slack()) else {
            return;
        };

        let config = &self.state.config().scheduler;

        let clients = workflows::low_stock::LowStockClients {
            pool: self.state.pool(),
            shopify,
            slack,
            threshold: config.low_stock_threshold,
            email_recipients: &config.low_stock_email_recipients,
        };

        workflows::low_stock::run(&clients).await;
    }

    /// Process the outbound email queue: send queued emails via M365.
    async fn process_outbound_queue(&self) {
        let mailbox = self.state.m365().mailboxes().first().cloned();
        let Some(mailbox) = mailbox else {
            tracing::warn!("no mailbox configured for outbound sending");
            return;
        };

        outbound::sender::process_queue(self.state.pool(), self.state.m365(), &mailbox).await;
    }

    /// Sync customer segments: classify, tag in Shopify, and sync to Klaviyo.
    async fn sync_customer_segments(&self) {
        let (Some(shopify), Some(klaviyo)) = (self.state.shopify(), self.state.klaviyo()) else {
            return;
        };

        let clients = workflows::segmentation::SegmentationClients {
            pool: self.state.pool(),
            shopify,
            klaviyo,
        };

        workflows::segmentation::run(&clients).await;
    }
}

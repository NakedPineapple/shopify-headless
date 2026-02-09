//! Task scheduler for the automations service.
//!
//! Runs periodic tasks using `tokio::select!` over interval ticks:
//! - Email polling (default: every 2 minutes)
//! - Shopify order/fulfillment polling (default: every 5 minutes)
//! - Abandoned cart detection (default: every 15 minutes)
//! - Low stock monitoring (default: every hour)
//! - Customer segment sync (default: daily)
//! - Subscription lifecycle (default: daily)
//!
//! Each workflow is protected by a circuit breaker. After 5 consecutive
//! failures, the workflow is paused for 10 minutes and a Slack alert is sent.

mod circuit_breaker;

use std::time::Duration;

use tokio::sync::watch;
use tokio::time::interval;

use crate::outbound;
use crate::state::AppState;
use crate::triage;
use crate::workflows;

use circuit_breaker::CircuitBreaker;

/// Run a workflow behind the circuit breaker.
///
/// Uses a macro instead of a generic async method to avoid higher-ranked
/// trait bound issues with async function pointers.
macro_rules! guarded {
    ($self:ident, $name:expr, $method:ident) => {
        if $self.breaker.is_allowed($name) {
            let ok = $self.$method().await;
            if ok {
                $self.breaker.record_success($name);
            } else {
                let tripped = $self.breaker.record_failure($name);
                if tripped {
                    tracing::error!(
                        workflow = $name,
                        failures = $self.breaker.failure_count($name),
                        "circuit breaker tripped — workflow paused for 10 minutes"
                    );
                    $self.send_breaker_alert($name).await;
                }
            }
        } else {
            tracing::debug!(
                workflow = $name,
                failures = $self.breaker.failure_count($name),
                "workflow paused by circuit breaker"
            );
        }
    };
}

/// Scheduler that runs periodic automation tasks.
pub struct Scheduler {
    state: AppState,
    breaker: CircuitBreaker,
}

impl Scheduler {
    /// Create a new scheduler.
    #[must_use]
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            breaker: CircuitBreaker::new(),
        }
    }

    /// Run the scheduler loop until a shutdown signal is received.
    ///
    /// Each tick calls the corresponding workflow handler. Errors in individual
    /// handlers are logged but do not stop the scheduler. A circuit breaker
    /// pauses workflows that fail repeatedly.
    pub async fn run(mut self, mut shutdown: watch::Receiver<bool>) {
        let config = &self.state.config().scheduler;

        let mut email_poll = interval(Duration::from_secs(config.email_poll_interval_secs));
        let mut order_poll = interval(Duration::from_secs(config.order_poll_interval_secs));
        let mut cart_check = interval(Duration::from_secs(config.cart_check_interval_secs));
        let mut stock_check = interval(Duration::from_secs(config.stock_check_interval_secs));
        let mut segment_sync = interval(Duration::from_secs(config.segment_sync_interval_secs));
        let mut subscription_check =
            interval(Duration::from_secs(config.subscription_check_interval_secs));

        tracing::info!(
            email_poll_secs = config.email_poll_interval_secs,
            order_poll_secs = config.order_poll_interval_secs,
            cart_check_secs = config.cart_check_interval_secs,
            stock_check_secs = config.stock_check_interval_secs,
            segment_sync_secs = config.segment_sync_interval_secs,
            subscription_check_secs = config.subscription_check_interval_secs,
            "scheduler started"
        );

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    tracing::info!("scheduler received shutdown signal");
                    break;
                }
                _ = email_poll.tick() => { guarded!(self, "email_poll", poll_emails); },
                _ = order_poll.tick() => { guarded!(self, "order_poll", poll_shopify_events); },
                _ = cart_check.tick() => { guarded!(self, "cart_check", check_abandoned_carts); },
                _ = stock_check.tick() => { guarded!(self, "stock_check", check_low_stock); },
                _ = segment_sync.tick() => { guarded!(self, "segment_sync", sync_customer_segments); },
                _ = subscription_check.tick() => { guarded!(self, "subscription_check", check_subscriptions); },
            }
        }

        tracing::info!("scheduler stopped");
    }

    /// Send a Slack alert when a circuit breaker trips.
    async fn send_breaker_alert(&self, workflow: &'static str) {
        use naked_pineapple_services::slack::{Block, ContextElement, PlainText, Text};

        let Some(slack) = self.state.slack() else {
            return;
        };

        let blocks = vec![
            Block::Header {
                text: PlainText::new("Circuit Breaker Tripped"),
            },
            Block::Section {
                text: Text::mrkdwn(format!(
                    "Workflow *{workflow}* has failed {} consecutive times and has been \
                     paused for 10 minutes.\n\nCheck logs for errors.",
                    self.breaker.failure_count(workflow),
                )),
                accessory: None,
            },
            Block::Context {
                elements: vec![ContextElement::Mrkdwn {
                    text: "Automated alert from automations circuit breaker".to_string(),
                }],
            },
        ];

        let channel = slack.default_channel();
        let fallback = format!("Circuit breaker tripped for workflow: {workflow}");

        if let Err(e) = slack.post_message(channel, blocks, Some(&fallback)).await {
            tracing::warn!(
                workflow = workflow,
                error = %e,
                "failed to send circuit breaker Slack alert"
            );
        }
    }

    /// Poll shared mailboxes for unread emails and run the triage pipeline.
    /// Returns `true` on success, `false` on failure.
    async fn poll_emails(&self) -> bool {
        let mailboxes = self.state.m365().mailboxes().to_vec();
        let mut any_failure = false;

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
                    any_failure = true;
                }
            }
        }

        !any_failure
    }

    /// Poll Shopify for recent orders/fulfillments and fire Klaviyo events.
    async fn poll_shopify_events(&self) -> bool {
        let (Some(shopify), Some(klaviyo)) = (self.state.shopify(), self.state.klaviyo()) else {
            return true;
        };

        let poll_minutes = self.state.config().scheduler.order_poll_interval_secs / 60 + 2;
        let pool = self.state.pool();

        outbound::poller::poll_new_orders(pool, shopify, klaviyo, poll_minutes).await;
        outbound::poller::poll_fulfillments(pool, shopify, klaviyo, poll_minutes).await;
        outbound::poller::poll_deliveries(pool, shopify, klaviyo, poll_minutes).await;
        true
    }

    /// Detect abandoned carts, trigger Klaviyo recovery flows, and check for recoveries.
    async fn check_abandoned_carts(&self) -> bool {
        let (Some(shopify), Some(klaviyo)) = (self.state.shopify(), self.state.klaviyo()) else {
            return true;
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
        true
    }

    /// Check for low stock items and send Slack/email alerts.
    async fn check_low_stock(&self) -> bool {
        let (Some(shopify), Some(slack)) = (self.state.shopify(), self.state.slack()) else {
            return true;
        };

        let config = &self.state.config().scheduler;

        let clients = workflows::low_stock::LowStockClients {
            pool: self.state.pool(),
            shopify,
            slack,
            email_service: self.state.email_service(),
            threshold: config.low_stock_threshold,
            email_recipients: &config.low_stock_email_recipients,
        };

        workflows::low_stock::run(&clients).await;
        true
    }

    /// Check subscription lifecycle: renewals, payment failures, and win-back.
    async fn check_subscriptions(&self) -> bool {
        let (Some(shopify), Some(klaviyo)) = (self.state.shopify(), self.state.klaviyo()) else {
            return true;
        };

        let config = &self.state.config().scheduler;

        let clients = workflows::subscription_lifecycle::SubscriptionClients {
            pool: self.state.pool(),
            shopify,
            klaviyo,
            renewal_reminder_days: config.subscription_renewal_reminder_days,
            winback_delay_days: config.subscription_winback_delay_days,
        };

        workflows::subscription_lifecycle::run(&clients).await;
        true
    }

    /// Sync customer segments: classify, tag in Shopify, and sync to Klaviyo.
    async fn sync_customer_segments(&self) -> bool {
        let (Some(shopify), Some(klaviyo)) = (self.state.shopify(), self.state.klaviyo()) else {
            return true;
        };

        let clients = workflows::segmentation::SegmentationClients {
            pool: self.state.pool(),
            shopify,
            klaviyo,
        };

        workflows::segmentation::run(&clients).await;
        true
    }
}

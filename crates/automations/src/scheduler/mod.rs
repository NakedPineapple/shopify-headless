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

use crate::amazon;
use crate::meta;
use crate::outbound;
use crate::pinterest;
use crate::state::AppState;
use crate::tiktok;
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
        let mut webhook_events = interval(Duration::from_secs(config.webhook_event_interval_secs));
        let mut summary_check = interval(Duration::from_secs(config.summary_check_interval_secs));
        let mut amazon_order_poll =
            interval(Duration::from_secs(config.amazon_order_poll_interval_secs));
        let mut meta_order_poll =
            interval(Duration::from_secs(config.meta_order_poll_interval_secs));
        let mut tiktok_order_poll =
            interval(Duration::from_secs(config.tiktok_order_poll_interval_secs));
        let mut tiktok_settlement_poll = interval(Duration::from_secs(
            config.tiktok_settlement_poll_interval_secs,
        ));
        let mut tiktok_return_poll =
            interval(Duration::from_secs(config.tiktok_return_poll_interval_secs));
        let mut tiktok_performance_poll = interval(Duration::from_secs(
            config.tiktok_performance_poll_interval_secs,
        ));
        let mut pinterest_conversion_poll = interval(Duration::from_secs(
            config.pinterest_conversion_poll_interval_secs,
        ));

        tracing::info!(
            email_poll_secs = config.email_poll_interval_secs,
            order_poll_secs = config.order_poll_interval_secs,
            amazon_order_poll_secs = config.amazon_order_poll_interval_secs,
            meta_order_poll_secs = config.meta_order_poll_interval_secs,
            tiktok_order_poll_secs = config.tiktok_order_poll_interval_secs,
            tiktok_settlement_poll_secs = config.tiktok_settlement_poll_interval_secs,
            tiktok_return_poll_secs = config.tiktok_return_poll_interval_secs,
            tiktok_performance_poll_secs = config.tiktok_performance_poll_interval_secs,
            pinterest_conversion_poll_secs = config.pinterest_conversion_poll_interval_secs,
            cart_check_secs = config.cart_check_interval_secs,
            stock_check_secs = config.stock_check_interval_secs,
            segment_sync_secs = config.segment_sync_interval_secs,
            subscription_check_secs = config.subscription_check_interval_secs,
            webhook_event_secs = config.webhook_event_interval_secs,
            summary_check_secs = config.summary_check_interval_secs,
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
                _ = webhook_events.tick() => { guarded!(self, "webhook_events", process_webhook_events); },
                _ = amazon_order_poll.tick() => { guarded!(self, "amazon_order_poll", poll_amazon_orders); },
                _ = meta_order_poll.tick() => { guarded!(self, "meta_order_poll", poll_meta_orders); },
                _ = tiktok_order_poll.tick() => { guarded!(self, "tiktok_order_poll", poll_tiktok_orders); },
                _ = tiktok_settlement_poll.tick() => { guarded!(self, "tiktok_settlement_poll", poll_tiktok_settlements); },
                _ = tiktok_return_poll.tick() => { guarded!(self, "tiktok_return_poll", poll_tiktok_returns); },
                _ = tiktok_performance_poll.tick() => { guarded!(self, "tiktok_performance_poll", poll_tiktok_performance); },
                _ = pinterest_conversion_poll.tick() => { guarded!(self, "pinterest_conversion_poll", sync_pinterest_conversions); },
                _ = summary_check.tick() => { guarded!(self, "summary_check", check_summaries); },
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

    /// Poll shared mailboxes for emails since the last sync and run the triage pipeline.
    /// Returns `true` on success, `false` on failure.
    async fn poll_emails(&self) -> bool {
        let mailboxes = self.state.m365().mailboxes().to_vec();
        let until = self.state.config().scheduler.email_sync_until.as_ref();
        let mut any_failure = false;

        for mailbox in &mailboxes {
            if !self.poll_single_mailbox(mailbox, until).await {
                any_failure = true;
            }
        }

        !any_failure
    }

    /// Poll a single mailbox using the high water mark approach.
    ///
    /// 1. Read the watermark from DB (default: 30 days ago)
    /// 2. Fetch folder map from M365
    /// 3. Fetch all messages since the watermark (optionally capped by `until`)
    /// 4. Process through the triage pipeline
    /// 5. Update the watermark to the newest `receivedDateTime` seen
    async fn poll_single_mailbox(
        &self,
        mailbox: &str,
        until: Option<&chrono::DateTime<chrono::Utc>>,
    ) -> bool {
        use crate::db::email_sync_state;

        let pool = self.state.pool();

        // 1. Get watermark (default: 30 days ago)
        let watermark = match email_sync_state::get_high_water_mark(pool, mailbox).await {
            Ok(Some(ts)) => {
                tracing::debug!(mailbox = %mailbox, %ts, "found watermark");
                ts
            }
            Ok(None) => {
                let default = match until {
                    Some(cap) if *cap < chrono::Utc::now() => *cap - chrono::Duration::days(30),
                    _ => chrono::Utc::now() - chrono::Duration::days(30),
                };
                tracing::info!(mailbox = %mailbox, %default, "no watermark found, using default");
                default
            }
            Err(e) => {
                tracing::error!(mailbox = %mailbox, error = %e, "failed to read watermark");
                return false;
            }
        };

        // 2. Fetch folder map
        let folder_map = match self.state.m365().list_folders(mailbox).await {
            Ok(map) => map,
            Err(e) => {
                tracing::error!(mailbox = %mailbox, error = %e, "failed to list folders");
                return false;
            }
        };

        // 3. Fetch all messages since watermark
        tracing::debug!(mailbox = %mailbox, %watermark, ?until, "fetching messages");
        let messages = match self
            .state
            .m365()
            .list_messages_since(mailbox, &watermark, until)
            .await
        {
            Ok(msgs) => msgs,
            Err(e) => {
                tracing::error!(mailbox = %mailbox, error = %e, "failed to fetch messages");
                return false;
            }
        };

        if messages.is_empty() {
            tracing::debug!(mailbox = %mailbox, %watermark, ?until, "no messages found");
            return true;
        }

        tracing::info!(
            mailbox = %mailbox,
            count = messages.len(),
            watermark = %watermark,
            "fetched messages since watermark"
        );

        // 4. Track max receivedDateTime for watermark update
        let max_received = messages.iter().filter_map(|m| m.received_date_time).max();

        // 5. Process through triage pipeline
        let clients = triage::TriageClients {
            pool,
            m365: self.state.m365(),
            claude: self.state.claude(),
            slack: self.state.slack(),
            shopify: self.state.shopify(),
        };

        triage::process_messages(&clients, mailbox, messages, &folder_map).await;

        // 6. Update watermark after successful processing
        if let Some(max_ts) = max_received
            && let Err(e) = email_sync_state::upsert_high_water_mark(pool, mailbox, max_ts).await
        {
            tracing::error!(mailbox = %mailbox, error = %e, "failed to update watermark");
            return false;
        }

        true
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

    /// Process pending webhook events from `admin.webhook_event`.
    async fn process_webhook_events(&self) -> bool {
        let clients = workflows::webhook_events::WebhookDispatchClients {
            pool: self.state.pool(),
            shopify: self.state.shopify(),
            klaviyo: self.state.klaviyo(),
            slack: self.state.slack(),
            storefront_url: self
                .state
                .config()
                .storefront_sync
                .as_ref()
                .map(|s| s.internal_url.as_str()),
        };
        workflows::webhook_events::run(&clients).await
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

    /// Poll Meta Commerce API for new orders and cache locally.
    async fn poll_meta_orders(&self) -> bool {
        let Some(client) = self.state.meta() else {
            return true;
        };

        meta::order_sync::poll_meta_orders(self.state.pool(), client).await
    }

    /// Poll TikTok Shop API for new orders and cache locally.
    async fn poll_tiktok_orders(&self) -> bool {
        let Some(client) = self.state.tiktok() else {
            return true;
        };
        tiktok::order_sync::poll_tiktok_orders(self.state.pool(), client).await
    }

    /// Poll TikTok Shop API for settlements and cache locally.
    async fn poll_tiktok_settlements(&self) -> bool {
        let Some(client) = self.state.tiktok() else {
            return true;
        };
        tiktok::settlement_sync::poll_tiktok_settlements(self.state.pool(), client).await
    }

    /// Poll TikTok Shop API for returns and cache locally.
    async fn poll_tiktok_returns(&self) -> bool {
        let Some(client) = self.state.tiktok() else {
            return true;
        };
        tiktok::return_sync::poll_tiktok_returns(self.state.pool(), client).await
    }

    /// Poll TikTok Shop API for performance metrics.
    async fn poll_tiktok_performance(&self) -> bool {
        let Some(client) = self.state.tiktok() else {
            return true;
        };
        tiktok::performance_sync::poll_tiktok_performance(self.state.pool(), client).await
    }

    /// Send recent Shopify orders as conversion events to Pinterest CAPI.
    async fn sync_pinterest_conversions(&self) -> bool {
        let (Some(shopify), Some(pinterest)) = (self.state.shopify(), self.state.pinterest())
        else {
            return true;
        };

        pinterest::conversion_sync::sync_conversions(shopify, pinterest).await
    }

    /// Poll Amazon SP-API for new orders and cache locally.
    async fn poll_amazon_orders(&self) -> bool {
        let Some(client) = self.state.amazon() else {
            return true;
        };

        amazon::order_sync::poll_amazon_orders(self.state.pool(), client).await
    }

    /// Check wall clock and fire daily/weekly summary emails if due.
    async fn check_summaries(&self) -> bool {
        use crate::db::summary_state;
        use chrono::{Datelike, Timelike, Utc};

        let config = &self.state.config().scheduler;

        // Skip if no recipients or required services are missing
        if config.summary_email_recipients.is_empty() {
            return true;
        }
        let (Some(shopify), Some(email_service)) =
            (self.state.shopify(), self.state.email_service())
        else {
            return true;
        };

        let now = Utc::now();
        let hour = u8::try_from(now.hour()).unwrap_or(0);
        let minute = u8::try_from(now.minute()).unwrap_or(0);
        let pool = self.state.pool();

        // Daily summary check
        if hour == config.daily_summary_hour
            && minute == config.daily_summary_minute
            && should_run(pool, "daily_summary", &now).await
        {
            let clients = workflows::business_summary::daily::DailySummaryClients {
                pool,
                shopify,
                email_service,
                recipients: &config.summary_email_recipients,
                low_stock_threshold: config.low_stock_threshold,
            };
            workflows::business_summary::daily::run(&clients).await;
            let _ = summary_state::record_run(pool, "daily_summary", now).await;
        }

        // Weekly summary check
        let today_day = now.weekday().to_string().to_lowercase();
        if today_day == config.weekly_summary_day
            && hour == config.weekly_summary_hour
            && minute == config.weekly_summary_minute
            && should_run(pool, "weekly_summary", &now).await
        {
            let clients = workflows::business_summary::weekly::WeeklySummaryClients {
                pool,
                shopify,
                email_service,
                recipients: &config.summary_email_recipients,
                low_stock_threshold: config.low_stock_threshold,
            };
            workflows::business_summary::weekly::run(&clients).await;
            let _ = summary_state::record_run(pool, "weekly_summary", now).await;
        }

        true
    }
}

/// Check whether a summary workflow should run, based on last-run time.
///
/// Returns `true` if the workflow has never run or last ran more than 23
/// hours ago (prevents double-runs within the same day).
async fn should_run(
    pool: &sqlx::PgPool,
    workflow: &str,
    now: &chrono::DateTime<chrono::Utc>,
) -> bool {
    use crate::db::summary_state;

    match summary_state::get_last_run(pool, workflow).await {
        Ok(Some(last)) => {
            let elapsed = *now - last;
            elapsed.num_hours() >= 23
        }
        Ok(None) => true,
        Err(e) => {
            tracing::warn!(
                workflow = workflow,
                error = %e,
                "failed to check summary last-run state, skipping"
            );
            false
        }
    }
}

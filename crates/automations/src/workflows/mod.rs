//! Automated workflow modules for the automations service.
//!
//! Each workflow runs on a schedule and handles a specific automation:
//! - Abandoned cart detection and recovery via Klaviyo flows
//! - Low stock monitoring with Slack alerts
//! - Customer segmentation with Shopify tagging and Klaviyo sync
//! - Webhook event processing (dispatches stored webhook events)

pub mod abandoned_cart;
pub mod business_summary;
pub mod low_stock;
pub mod segmentation;
pub mod subscription_lifecycle;
pub mod webhook_events;

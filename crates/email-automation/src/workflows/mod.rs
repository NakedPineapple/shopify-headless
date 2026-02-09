//! Automated workflow modules for the email automation service.
//!
//! Each workflow runs on a schedule and handles a specific automation:
//! - Abandoned cart detection and recovery via Klaviyo flows
//! - Low stock monitoring with Slack alerts
//! - Customer segmentation with Shopify tagging and Klaviyo sync

pub mod abandoned_cart;
pub mod low_stock;
pub mod segmentation;

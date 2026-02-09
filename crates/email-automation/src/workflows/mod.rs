//! Automated workflow modules for the email automation service.
//!
//! Each workflow runs on a schedule and handles a specific automation:
//! - Abandoned cart detection and recovery via Klaviyo flows
//! - Low stock alerts (Phase 5)
//! - Customer segmentation (Phase 5)

pub mod abandoned_cart;

//! Shared service clients for Naked Pineapple.
//!
//! This library provides API clients and configuration types used by both
//! the admin panel and the automations binary:
//!
//! - [`claude`] - Anthropic Claude API client (streaming + non-streaming)
//! - [`klaviyo`] - Klaviyo API client for newsletter and helpdesk
//! - [`slack`] - Slack Web API client for notifications and approvals
//! - [`email`] - SMTP email delivery via lettre
//! - [`config`] - Shared configuration types loaded from environment variables

#![cfg_attr(not(test), forbid(unsafe_code))]

pub mod claude;
pub mod config;
pub mod email;
pub mod klaviyo;
pub mod slack;

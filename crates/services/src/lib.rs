//! Shared service clients for Naked Pineapple.
//!
//! This library provides API clients and configuration types used by both
//! the admin panel and the automations binary:
//!
//! - [`claude`] - Anthropic Claude API client (streaming + non-streaming)
//! - [`klaviyo`] - Klaviyo API client for newsletter and helpdesk
//! - [`openai`] - `OpenAI` embedding client for semantic similarity search
//! - [`slack`] - Slack Web API client for notifications and approvals
//! - [`email`] - SMTP email delivery via lettre
//! - [`judgeme`] - Judge.me API client for product reviews
//! - [`microsoft_graph`] - Microsoft Graph API client for M365 mail operations
//! - [`config`] - Shared configuration types loaded from environment variables

#![cfg_attr(not(test), forbid(unsafe_code))]

pub mod amazon_sp;
pub mod claude;
pub mod config;
pub mod email;
pub mod judgeme;
pub mod klaviyo;
pub mod microsoft_graph;
pub mod openai;
pub mod slack;

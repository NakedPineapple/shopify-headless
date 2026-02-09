//! Microsoft Graph API client for shared mailbox operations.
//!
//! Provides authenticated access to Microsoft 365 shared mailboxes using
//! the `OAuth2` client credentials flow (application permissions).
//!
//! # Required Azure AD Permissions
//!
//! The application registration must have `Mail.ReadWrite` and `Mail.Send`
//! application permissions granted on the shared mailbox.

pub mod auth;
pub mod client;
pub mod error;
pub mod mail;
pub mod types;

pub use client::M365Client;
pub use error::M365Error;
pub use types::GraphMessage;

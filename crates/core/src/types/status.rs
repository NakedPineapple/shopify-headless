//! Status enums for various entities.
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use serde::{Deserialize, Serialize};
//!
//! /// Order fulfillment status (from Shopify).
//! #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
//! #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
//! pub enum FulfillmentStatus {
//!     Unfulfilled,
//!     PartiallyFulfilled,
//!     Fulfilled,
//!     Restocked,
//!     PendingFulfillment,
//!     Open,
//!     InProgress,
//!     OnHold,
//!     Scheduled,
//! }
//!
//! /// Order financial status (from Shopify).
//! #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
//! #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
//! pub enum FinancialStatus {
//!     Pending,
//!     Authorized,
//!     PartiallyPaid,
//!     Paid,
//!     PartiallyRefunded,
//!     Refunded,
//!     Voided,
//! }
//!
//! /// User email verification status.
//! #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
//! pub enum EmailVerificationStatus {
//!     Unverified,
//!     Pending,
//!     Verified,
//! }
//!
//! /// Chat message role (for Claude integration).
//! #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
//! #[serde(rename_all = "snake_case")]
//! pub enum ChatRole {
//!     User,
//!     Assistant,
//!     ToolUse,
//!     ToolResult,
//! }
//! ```

use serde::{Deserialize, Serialize};

/// Order fulfillment status.
///
/// Maps to Shopify's fulfillment status values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FulfillmentStatus {
    #[default]
    Unfulfilled,
    PartiallyFulfilled,
    Fulfilled,
    Restocked,
}

/// Order financial status.
///
/// Maps to Shopify's financial status values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinancialStatus {
    #[default]
    Pending,
    Authorized,
    PartiallyPaid,
    Paid,
    PartiallyRefunded,
    Refunded,
    Voided,
}

/// Email verification status for users.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmailVerificationStatus {
    #[default]
    Unverified,
    Pending,
    Verified,
}

/// Chat message role for Claude AI integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    User,
    Assistant,
    ToolUse,
    ToolResult,
}

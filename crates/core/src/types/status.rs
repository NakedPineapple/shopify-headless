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
#[cfg_attr(feature = "postgres", derive(sqlx::Type))]
#[cfg_attr(
    feature = "postgres",
    sqlx(type_name = "admin.chat_role", rename_all = "snake_case")
)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    User,
    Assistant,
    ToolUse,
    ToolResult,
}

/// Admin role with different permission levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "postgres", derive(sqlx::Type))]
#[cfg_attr(
    feature = "postgres",
    sqlx(type_name = "admin.admin_role", rename_all = "snake_case")
)]
#[serde(rename_all = "snake_case")]
pub enum AdminRole {
    /// Full access to all admin features including user management.
    SuperAdmin,
    /// Full access to store management features.
    Admin,
    /// Read-only access to store data.
    Viewer,
}

impl std::fmt::Display for AdminRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SuperAdmin => write!(f, "super_admin"),
            Self::Admin => write!(f, "admin"),
            Self::Viewer => write!(f, "viewer"),
        }
    }
}

impl std::str::FromStr for AdminRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "super_admin" => Ok(Self::SuperAdmin),
            "admin" => Ok(Self::Admin),
            "viewer" => Ok(Self::Viewer),
            _ => Err(format!("invalid admin role: {s}")),
        }
    }
}

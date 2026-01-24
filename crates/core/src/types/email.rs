//! Validated email address type.
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use serde::{Deserialize, Serialize};
//! use std::str::FromStr;
//!
//! #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
//! #[serde(try_from = "String", into = "String")]
//! pub struct Email(String);
//!
//! impl Email {
//!     /// Create a new email, validating the format.
//!     pub fn new(email: impl Into<String>) -> Result<Self, EmailError> {
//!         let email = email.into().to_lowercase().trim().to_string();
//!
//!         // Basic validation: contains @ and has parts on both sides
//!         let parts: Vec<&str> = email.split('@').collect();
//!         if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
//!             return Err(EmailError::InvalidFormat);
//!         }
//!
//!         // Domain must have at least one dot
//!         if !parts[1].contains('.') {
//!             return Err(EmailError::InvalidDomain);
//!         }
//!
//!         Ok(Self(email))
//!     }
//!
//!     pub fn as_str(&self) -> &str {
//!         &self.0
//!     }
//!
//!     pub fn domain(&self) -> &str {
//!         self.0.split('@').nth(1).unwrap_or("")
//!     }
//!
//!     pub fn local_part(&self) -> &str {
//!         self.0.split('@').next().unwrap_or("")
//!     }
//! }
//!
//! #[derive(Debug, Clone, thiserror::Error)]
//! pub enum EmailError {
//!     #[error("Invalid email format")]
//!     InvalidFormat,
//!     #[error("Invalid domain")]
//!     InvalidDomain,
//! }
//!
//! // SQLx support
//! impl<'r> sqlx::Decode<'r, sqlx::Postgres> for Email { ... }
//! impl sqlx::Type<sqlx::Postgres> for Email { ... }
//! ```

use serde::{Deserialize, Serialize};

/// A validated email address.
///
/// TODO: Implement full validation and `SQLx` support.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Email(String);

impl Email {
    /// Create a new email without validation.
    ///
    /// TODO: Add proper validation.
    #[must_use]
    pub fn new_unchecked(email: impl Into<String>) -> Self {
        Self(email.into().to_lowercase())
    }

    /// Get the email as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Email {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

//! Type-safe price representation using decimal arithmetic.
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use rust_decimal::Decimal;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
//! pub struct Price {
//!     /// Amount in the smallest currency unit (e.g., cents for USD).
//!     amount: Decimal,
//!     /// ISO 4217 currency code (e.g., "USD", "EUR").
//!     currency: Currency,
//! }
//!
//! impl Price {
//!     pub fn new(amount: Decimal, currency: Currency) -> Self {
//!         Self { amount, currency }
//!     }
//!
//!     pub fn from_cents(cents: i64, currency: Currency) -> Self {
//!         Self {
//!             amount: Decimal::new(cents, 2),
//!             currency,
//!         }
//!     }
//!
//!     pub fn amount(&self) -> Decimal { self.amount }
//!     pub fn currency(&self) -> Currency { self.currency }
//!
//!     /// Format for display (e.g., "$19.99").
//!     pub fn display(&self) -> String {
//!         format!("{}{:.2}", self.currency.symbol(), self.amount)
//!     }
//! }
//!
//! #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
//! pub enum Currency {
//!     USD,
//!     EUR,
//!     GBP,
//!     CAD,
//!     AUD,
//! }
//!
//! impl Currency {
//!     pub fn symbol(&self) -> &'static str {
//!         match self {
//!             Self::USD | Self::CAD | Self::AUD => "$",
//!             Self::EUR => "€",
//!             Self::GBP => "£",
//!         }
//!     }
//!
//!     pub fn code(&self) -> &'static str {
//!         match self {
//!             Self::USD => "USD",
//!             Self::EUR => "EUR",
//!             Self::GBP => "GBP",
//!             Self::CAD => "CAD",
//!             Self::AUD => "AUD",
//!         }
//!     }
//! }
//! ```

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// A price with currency information.
///
/// TODO: Implement full currency support and formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Price {
    /// Amount in the currency's standard unit (e.g., dollars, not cents).
    pub amount: Decimal,
    /// ISO 4217 currency code.
    pub currency_code: CurrencyCode,
}

impl Price {
    /// Create a new price.
    #[must_use]
    pub const fn new(amount: Decimal, currency_code: CurrencyCode) -> Self {
        Self {
            amount,
            currency_code,
        }
    }
}

/// ISO 4217 currency codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum CurrencyCode {
    #[default]
    USD,
    EUR,
    GBP,
    CAD,
    AUD,
}

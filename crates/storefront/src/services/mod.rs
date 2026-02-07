//! Business logic services for storefront.
//!
//! # Services
//!
//! - `discount_matcher` - Discount qualifying rule evaluation
//! - `klaviyo` - Klaviyo API for subscription management
//! - `mixpanel` - Analytics event tracking

pub mod discount_matcher;
mod klaviyo;
pub mod mixpanel;

pub use discount_matcher::{DiscountSuggestion, match_qualifying_rules};
pub use klaviyo::{KlaviyoClient, KlaviyoError};
pub use mixpanel::MixpanelClient;

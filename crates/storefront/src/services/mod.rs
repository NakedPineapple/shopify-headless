//! Business logic services for storefront.
//!
//! # Services
//!
//! - `auth` - User authentication (password, `WebAuthn`, OAuth)
//! - `email` - Email sending (verification, password reset)
//! - `cart` - Cart operations (wrapper around Shopify cart)
//! - `analytics` - Analytics event tracking
//! - `klaviyo` - Klaviyo API for subscription management
//! - `discount_matcher` - Discount qualifying rule evaluation

pub mod auth;
pub mod discount_matcher;
mod klaviyo;
pub mod mixpanel;

pub use auth::{AuthError, AuthService};
pub use discount_matcher::{DiscountSuggestion, match_qualifying_rules};
pub use klaviyo::{KlaviyoClient, KlaviyoError};
pub use mixpanel::MixpanelClient;

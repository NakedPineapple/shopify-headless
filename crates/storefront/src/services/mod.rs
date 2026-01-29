//! Business logic services for storefront.
//!
//! # Services
//!
//! - `auth` - User authentication (password, `WebAuthn`, OAuth)
//! - `email` - Email sending (verification, password reset)
//! - `cart` - Cart operations (wrapper around Shopify cart)
//! - `analytics` - Analytics event tracking
//! - `klaviyo` - Klaviyo API for subscription management

pub mod auth;
mod klaviyo;

pub use auth::{AuthError, AuthService};
pub use klaviyo::{KlaviyoClient, KlaviyoError};

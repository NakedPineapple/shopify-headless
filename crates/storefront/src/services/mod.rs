//! Business logic services for storefront.
//!
//! # Services
//!
//! - `auth` - User authentication (password, `WebAuthn`, OAuth)
//! - `email` - Email sending (verification, password reset)
//! - `cart` - Cart operations (wrapper around Shopify cart)
//! - `analytics` - Analytics event tracking

pub mod auth;

pub use auth::{AuthError, AuthService};

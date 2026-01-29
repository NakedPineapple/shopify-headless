//! HTTP middleware stack for storefront.
//!
//! # Middleware Order (bottom to top in Router)
//!
//! 1. Sentry layer (capture errors)
//! 2. `TraceLayer` (request tracing)
//! 3. Request ID (add unique ID to each request)
//! 4. Session layer (tower-sessions with `PostgreSQL` store)
//! 5. User context (add user info to tracing span)
//! 6. Security headers (CSP, HSTS, etc.)
//! 7. Rate limiting (governor)

pub mod auth;
pub mod rate_limit;
pub mod session;
pub mod shopify_customer;

pub use auth::{OptionalAuth, RequireAuth, clear_current_customer, set_current_customer};
pub use rate_limit::{api_rate_limiter, auth_rate_limiter};
pub use session::create_session_layer;
pub use shopify_customer::{
    OptionalShopifyCustomer, RequireShopifyCustomer, clear_shopify_customer_token,
    set_shopify_customer_token,
};

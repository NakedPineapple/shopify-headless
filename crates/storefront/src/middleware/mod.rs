//! HTTP middleware stack for storefront.
//!
//! # Middleware Order (bottom to top in Router)
//!
//! 1. Sentry layer (capture errors)
//! 2. `TraceLayer` (request tracing)
//! 3. Request ID (add unique ID to each request)
//! 4. CSP nonce (generate per-request nonce for inline scripts)
//! 5. Session layer (tower-sessions with `PostgreSQL` store)
//! 6. User context (add user info to tracing span)
//! 7. Security headers (CSP, HSTS, etc.)
//! 8. Rate limiting (governor)

pub mod auth;
pub mod csp;
pub mod rate_limit;
pub mod request_id;
pub mod security_headers;
pub mod session;
pub mod shopify_customer;

pub use auth::{OptionalAuth, RequireAuth, clear_current_customer, set_current_customer};
pub use csp::{CspNonce, csp_nonce_middleware};
pub use rate_limit::{api_rate_limiter, auth_rate_limiter};
pub use request_id::request_id_middleware;
pub use security_headers::security_headers_middleware;
pub use session::create_session_layer;
pub use shopify_customer::{
    OptionalShopifyCustomer, RequireShopifyCustomer, clear_shopify_customer_token,
    set_shopify_customer_token,
};

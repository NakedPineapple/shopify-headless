//! HTTP middleware stack for admin.
//!
//! # Middleware Order (bottom to top in Router)
//!
//! 1. Sentry layer (capture errors)
//! 2. `TraceLayer` (request tracing)
//! 3. Request ID (add unique ID to each request)
//! 4. Session layer (tower-sessions with `PostgreSQL` store)
//! 5. Admin context (add admin user info to tracing span)
//! 6. Security headers (stricter CSP for admin)
//! 7. Auth guard (require authentication for most routes)

pub mod auth;
pub mod session;

pub use auth::{
    OptionalAdminAuth, RequireAdminAuth, RequireSuperAdmin, clear_current_admin,
    require_super_admin, set_current_admin,
};
pub use session::create_session_layer;

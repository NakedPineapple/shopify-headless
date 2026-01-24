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
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use axum::Router;
//! use tower::ServiceBuilder;
//! use tower_http::trace::TraceLayer;
//! use tower_sessions::{SessionManagerLayer, PostgresStore};
//!
//! pub mod rate_limit;
//! pub mod request_id;
//! pub mod security_headers;
//! pub mod user_context;
//!
//! pub fn stack(
//!     session_store: PostgresStore,
//!     session_secret: &[u8],
//! ) -> impl tower::Layer<...> {
//!     ServiceBuilder::new()
//!         // Error tracking (outermost)
//!         .layer(sentry_tower::NewSentryLayer::new_from_top())
//!         .layer(sentry_tower::SentryHttpLayer::new())
//!
//!         // Request tracing
//!         .layer(TraceLayer::new_for_http())
//!
//!         // Request ID for correlation
//!         .layer(axum::middleware::from_fn(request_id::middleware))
//!
//!         // Sessions
//!         .layer(
//!             SessionManagerLayer::new(session_store)
//!                 .with_secure(true)
//!                 .with_same_site(tower_sessions::cookie::SameSite::Lax)
//!                 .with_http_only(true)
//!         )
//!
//!         // User context for tracing
//!         .layer(axum::middleware::from_fn(user_context::middleware))
//!
//!         // Security headers
//!         .layer(axum::middleware::from_fn(security_headers::middleware))
//!
//!         // Rate limiting (innermost)
//!         .layer(rate_limit::layer())
//! }
//!
//! // security_headers.rs
//! pub async fn middleware(
//!     request: Request,
//!     next: Next,
//! ) -> Response {
//!     let mut response = next.run(request).await;
//!     let headers = response.headers_mut();
//!
//!     headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
//!     headers.insert("X-Frame-Options", "DENY".parse().unwrap());
//!     headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
//!     headers.insert(
//!         "Content-Security-Policy",
//!         "default-src 'self'; script-src 'self' 'unsafe-inline' ...".parse().unwrap()
//!     );
//!     headers.insert(
//!         "Strict-Transport-Security",
//!         "max-age=31536000; includeSubDomains".parse().unwrap()
//!     );
//!
//!     response
//! }
//!
//! // request_id.rs
//! pub async fn middleware(
//!     mut request: Request,
//!     next: Next,
//! ) -> Response {
//!     let request_id = uuid::Uuid::new_v4().to_string();
//!     request.extensions_mut().insert(RequestId(request_id.clone()));
//!
//!     let mut response = next.run(request).await;
//!     response.headers_mut().insert("X-Request-ID", request_id.parse().unwrap());
//!
//!     response
//! }
//! ```

// TODO: Implement middleware

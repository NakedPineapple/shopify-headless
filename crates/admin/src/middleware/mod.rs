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
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use axum::{extract::State, middleware::Next, response::Response, http::Request};
//! use tower::ServiceBuilder;
//! use tower_http::trace::TraceLayer;
//! use tower_sessions::{SessionManagerLayer, PostgresStore};
//!
//! pub mod admin_context;
//! pub mod auth_guard;
//! pub mod request_id;
//! pub mod security_headers;
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
//!                 .with_same_site(tower_sessions::cookie::SameSite::Strict)
//!                 .with_http_only(true)
//!         )
//!
//!         // Admin context for tracing
//!         .layer(axum::middleware::from_fn(admin_context::middleware))
//!
//!         // Security headers (stricter for admin)
//!         .layer(axum::middleware::from_fn(security_headers::middleware))
//! }
//!
//! // auth_guard.rs - Apply to protected routes
//! pub async fn require_auth<B>(
//!     State(state): State<AppState>,
//!     session: Session,
//!     request: Request<B>,
//!     next: Next<B>,
//! ) -> Result<Response, AppError> {
//!     let admin_user_id = session
//!         .get::<i64>("admin_user_id")
//!         .await?
//!         .ok_or(AppError::Unauthorized("Not logged in".to_string()))?;
//!
//!     // Verify user still exists and is active
//!     let admin_user = db::admin_users::get_by_id(state.pool(), admin_user_id)
//!         .await?
//!         .ok_or(AppError::Unauthorized("User not found".to_string()))?;
//!
//!     // Add to request extensions
//!     request.extensions_mut().insert(admin_user);
//!
//!     Ok(next.run(request).await)
//! }
//!
//! // security_headers.rs - Stricter CSP for admin
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
//!     // Stricter CSP - no external scripts
//!     headers.insert(
//!         "Content-Security-Policy",
//!         "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'".parse().unwrap()
//!     );
//!     headers.insert(
//!         "Strict-Transport-Security",
//!         "max-age=31536000; includeSubDomains; preload".parse().unwrap()
//!     );
//!
//!     response
//! }
//! ```

// TODO: Implement middleware

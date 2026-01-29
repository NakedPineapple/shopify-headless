//! Request ID middleware for request tracing and correlation.
//!
//! Generates a UUID v4 for each request if not provided by an upstream proxy
//! (e.g., Cloudflare, load balancer). The request ID is:
//! - Recorded in the current tracing span
//! - Added to the Sentry scope for error correlation
//! - Returned in the response headers

use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
use tracing::Span;
use uuid::Uuid;

/// The HTTP header name for request IDs.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Middleware that ensures every request has a unique request ID.
///
/// If the incoming request has an `x-request-id` header (from Cloudflare, a load
/// balancer, or another upstream proxy), that value is used. Otherwise, a new
/// UUID v4 is generated.
///
/// The request ID is:
/// 1. Recorded in the current tracing span via `Span::current().record()`
/// 2. Added to the Sentry scope as a tag for error correlation
/// 3. Added to the response headers for client visibility
pub async fn request_id_middleware(request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|h| h.to_str().ok())
        .map_or_else(|| Uuid::new_v4().to_string(), String::from);

    // Record in current span for structured logging
    Span::current().record("request_id", &request_id);

    // Set in Sentry scope for error correlation
    sentry::configure_scope(|scope| {
        scope.set_tag("request_id", &request_id);
    });

    let mut response = next.run(request).await;

    // Add to response headers so clients can reference the request ID
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }

    response
}

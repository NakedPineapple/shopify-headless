//! Security headers middleware for the admin panel.
//!
//! Adds security headers to all responses including HSTS for HTTPS enforcement.

use axum::{
    extract::Request,
    http::{
        HeaderName, HeaderValue,
        header::{
            CONTENT_SECURITY_POLICY, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
        },
    },
    middleware::Next,
    response::Response,
};

/// Add security headers to all responses.
///
/// Headers applied:
/// - `X-Frame-Options: DENY` - Prevent clickjacking
/// - `X-Content-Type-Options: nosniff` - Prevent MIME sniffing
/// - `Referrer-Policy: no-referrer` - Zero referrer leakage
/// - `Strict-Transport-Security` - Enforce HTTPS
/// - `Content-Security-Policy` - Restrictive CSP for admin
pub async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    // Prevent clickjacking
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));

    // Prevent MIME sniffing
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));

    // Zero referrer leakage
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));

    // HSTS: Enforce HTTPS for 1 year, including subdomains
    // Browsers ignore this header on HTTP connections, so it's safe to always include
    headers.insert(
        HeaderName::from_static("strict-transport-security"),
        HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
    );

    // Restrictive CSP for admin panel
    // Admin uses HTMX and Phosphor Icons, no external analytics
    headers.insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; \
             script-src 'self' 'unsafe-eval'; \
             style-src 'self' 'unsafe-inline'; \
             font-src 'self' data:; \
             img-src 'self' data: https://cdn.shopify.com https://images.nakedpineapple.co; \
             connect-src 'self'; \
             frame-src 'none'; \
             object-src 'none'; \
             base-uri 'self'; \
             form-action 'self'; \
             frame-ancestors 'none'",
        ),
    );

    // Prevent caching of sensitive admin responses
    headers.insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static("no-store, max-age=0"),
    );

    response
}

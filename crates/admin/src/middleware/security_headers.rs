//! Security headers middleware for the admin panel.
//!
//! Adds security headers to all responses including HSTS for HTTPS enforcement.

use axum::{
    extract::Request,
    http::{
        HeaderName, HeaderValue,
        header::{REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS},
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
/// - `Cache-Control: no-store` - Prevent caching of sensitive data
///
/// Note: No CSP is applied because the admin panel runs behind Tailscale VPN.
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

    // No CSP for admin panel - it runs behind Tailscale VPN with MDM verification.
    // Network-level security makes CSP redundant, and it interferes with inline
    // scripts needed for admin UI functionality.

    // Prevent caching of sensitive admin responses
    headers.insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static("no-store, max-age=0"),
    );

    response
}

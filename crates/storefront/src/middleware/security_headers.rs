//! Security headers middleware for XSS, clickjacking, and isolation protection.
//!
//! Adds restrictive security headers to all responses. Start locked down and
//! loosen only when specific functionality requires it.

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
/// - `Content-Security-Policy` - Strict CSP (see below)
/// - `Permissions-Policy` - Deny all sensitive features
/// - `Cache-Control: no-store, max-age=0` - Prevent caching sensitive data
/// - `Cross-Origin-Opener-Policy: same-origin` - Process isolation
/// - `Cross-Origin-Resource-Policy: same-origin` - Resource isolation
/// - `Cross-Origin-Embedder-Policy: require-corp` - Strict isolation
/// - `X-DNS-Prefetch-Control: off` - Prevent DNS prefetch leakage
///
/// # CSP Policy
///
/// Starting with maximum restriction - loosen only when needed:
/// ```text
/// default-src 'none';
/// script-src 'self';
/// style-src 'self';
/// font-src 'self';
/// img-src 'self' https://cdn.shopify.com;
/// connect-src 'self';
/// frame-src 'none';
/// object-src 'none';
/// base-uri 'self';
/// form-action 'self';
/// frame-ancestors 'none';
/// upgrade-insecure-requests
/// ```
pub async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    // Prevent clickjacking
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));

    // Prevent MIME sniffing
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));

    // Zero referrer leakage (stricter than same-origin)
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));

    // Strict CSP - start locked down, loosen only when needed
    headers.insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; \
             script-src 'self'; \
             style-src 'self'; \
             font-src 'self'; \
             img-src 'self' https://cdn.shopify.com; \
             connect-src 'self'; \
             frame-src 'none'; \
             object-src 'none'; \
             base-uri 'self'; \
             form-action 'self'; \
             frame-ancestors 'none'; \
             upgrade-insecure-requests",
        ),
    );

    // Strict Permissions Policy - deny all sensitive features
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(
            "accelerometer=(), \
             ambient-light-sensor=(), \
             autoplay=(), \
             battery=(), \
             browsing-topics=(), \
             camera=(), \
             cross-origin-isolated=(), \
             display-capture=(), \
             document-domain=(), \
             encrypted-media=(), \
             execution-while-not-rendered=(), \
             execution-while-out-of-viewport=(), \
             fullscreen=(), \
             geolocation=(), \
             gyroscope=(), \
             hid=(), \
             idle-detection=(), \
             interest-cohort=(), \
             magnetometer=(), \
             microphone=(), \
             midi=(), \
             navigation-override=(), \
             payment=(), \
             picture-in-picture=(), \
             publickey-credentials-get=(), \
             screen-wake-lock=(), \
             serial=(), \
             sync-xhr=(), \
             usb=(), \
             web-share=(), \
             xr-spatial-tracking=()",
        ),
    );

    // Prevent caching of sensitive responses
    headers.insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static("no-store, max-age=0"),
    );

    // Cross-Origin policies for additional isolation
    headers.insert(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );

    headers.insert(
        HeaderName::from_static("cross-origin-resource-policy"),
        HeaderValue::from_static("same-origin"),
    );

    // Strict COEP - require-corp for maximum isolation
    // Note: This may block cross-origin resources that don't set CORP headers.
    // If Shopify CDN images break, switch to "credentialless".
    headers.insert(
        HeaderName::from_static("cross-origin-embedder-policy"),
        HeaderValue::from_static("require-corp"),
    );

    // Prevent DNS prefetching to avoid leaking which links user hovers over
    headers.insert(
        HeaderName::from_static("x-dns-prefetch-control"),
        HeaderValue::from_static("off"),
    );

    response
}

//! Security headers middleware for XSS, clickjacking, and isolation protection.
//!
//! Adds restrictive security headers to all responses. CSP is dynamically built
//! with a per-request nonce for inline scripts and allowlisted analytics domains.

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

use super::csp::CspNonce;

// =============================================================================
// External domains for analytics and tracking
// =============================================================================

/// External script sources for analytics platforms and Shopify.
const SCRIPT_SRC_EXTERNAL: &[&str] = &[
    "https://www.googletagmanager.com",
    "https://www.google-analytics.com",
    "https://connect.facebook.net",
    "https://analytics.tiktok.com",
    "https://s.pinimg.com",
    "https://sc-static.net",
    "https://bat.bing.com",
    "https://static.ads-twitter.com",
    "https://cdn.mxpnl.com",
    "https://script.crazyegg.com",
    "https://static.cloudflareinsights.com",
    // Shop Pay web components
    "https://cdn.shopify.com",
];

/// External image sources for CDN and tracking pixels.
const IMG_SRC_EXTERNAL: &[&str] = &[
    "https://images.nakedpineapple.co",
    "https://cdn.shopify.com",
    "https://www.facebook.com",
    "https://www.google-analytics.com",
    "https://googleads.g.doubleclick.net",
    "https://ct.pinterest.com",
    "https://t.co",
    "https://analytics.twitter.com",
    "data:",
];

/// External connect sources for analytics beacons and Shopify.
const CONNECT_SRC_EXTERNAL: &[&str] = &[
    "https://www.google-analytics.com",
    "https://analytics.google.com",
    "https://region1.google-analytics.com",
    "https://www.facebook.com",
    "https://connect.facebook.net",
    "https://analytics.tiktok.com",
    "https://googleads.g.doubleclick.net",
    "https://cloudflareinsights.com",
    // Shop Pay APIs
    "https://shop.app",
    "https://*.shopify.com",
];

/// External frame sources for embedded widgets.
const FRAME_SRC_EXTERNAL: &[&str] = &[
    // Shop Pay payment terms iframe
    "https://cdn.shopify.com",
    "https://shop.app",
];

// =============================================================================
// Middleware
// =============================================================================

/// Add security headers to all responses.
///
/// Headers applied:
/// - `X-Frame-Options: DENY` - Prevent clickjacking
/// - `X-Content-Type-Options: nosniff` - Prevent MIME sniffing
/// - `Referrer-Policy: no-referrer` - Zero referrer leakage
/// - `Content-Security-Policy` - Dynamic CSP with nonce and analytics domains
/// - `Permissions-Policy` - Deny all sensitive features
/// - `Cache-Control: no-store, max-age=0` - Prevent caching sensitive data
/// - `Cross-Origin-Opener-Policy: same-origin` - Process isolation
/// - `Cross-Origin-Resource-Policy: same-origin` - Resource isolation
/// - `Cross-Origin-Embedder-Policy: credentialless` - Allow CORS resources
/// - `X-DNS-Prefetch-Control: off` - Prevent DNS prefetch leakage
pub async fn security_headers_middleware(request: Request, next: Next) -> Response {
    // Extract nonce BEFORE running the handler (it's set by csp_nonce_middleware)
    let nonce = request
        .extensions()
        .get::<CspNonce>()
        .map(|n| n.value().to_string())
        .unwrap_or_default();

    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    // Prevent clickjacking
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));

    // Prevent MIME sniffing
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));

    // Zero referrer leakage (stricter than same-origin)
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));

    // Dynamic CSP with nonce for inline scripts and analytics domains
    let csp = build_csp(&nonce);
    if let Ok(value) = HeaderValue::from_str(&csp) {
        headers.insert(CONTENT_SECURITY_POLICY, value);
    }

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

    // COEP: credentialless allows cross-origin resources with CORS headers
    // (Shopify CDN, analytics scripts, etc. work without requiring CORP headers)
    headers.insert(
        HeaderName::from_static("cross-origin-embedder-policy"),
        HeaderValue::from_static("credentialless"),
    );

    // Prevent DNS prefetching to avoid leaking which links user hovers over
    headers.insert(
        HeaderName::from_static("x-dns-prefetch-control"),
        HeaderValue::from_static("off"),
    );

    response
}

/// Build the Content-Security-Policy header value with the given nonce.
fn build_csp(nonce: &str) -> String {
    let script_src = SCRIPT_SRC_EXTERNAL.join(" ");
    let img_src = IMG_SRC_EXTERNAL.join(" ");
    let connect_src = CONNECT_SRC_EXTERNAL.join(" ");
    let frame_src = FRAME_SRC_EXTERNAL.join(" ");

    // Note: 'unsafe-eval' is required for HTMX to function (uses Function() internally).
    // All interactive behavior uses event delegation via data-action attributes,
    // avoiding the need for inline event handlers and 'unsafe-hashes'.
    format!(
        "default-src 'none'; \
         script-src 'self' 'nonce-{nonce}' 'unsafe-eval' {script_src}; \
         style-src 'self' 'unsafe-inline'; \
         font-src 'self' data:; \
         img-src 'self' {img_src}; \
         connect-src 'self' {connect_src}; \
         frame-src {frame_src}; \
         object-src 'none'; \
         base-uri 'self'; \
         form-action 'self'; \
         frame-ancestors 'none'; \
         upgrade-insecure-requests"
    )
}

//! Security headers middleware for XSS, clickjacking, and isolation protection.
//!
//! Adds restrictive security headers to all responses. CSP is dynamically built
//! with a per-request nonce for inline scripts and allowlisted domains.
//! When the browser sends `Sec-GPC: 1`, analytics domains are excluded from CSP.

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
// Essential external domains (always allowed)
// =============================================================================

/// Script sources needed regardless of GPC (Shopify Shop Pay, Turnstile).
const SCRIPT_SRC_ESSENTIAL: &[&str] = &[
    "https://cdn.shopify.com",
    "https://challenges.cloudflare.com",
];

/// Image sources needed regardless of GPC (CDN, Shopify).
const IMG_SRC_ESSENTIAL: &[&str] = &[
    "https://images.nakedpineapple.co",
    "https://cdn.shopify.com",
    "data:",
];

/// Connect sources needed regardless of GPC (Shopify, Sentry).
const CONNECT_SRC_ESSENTIAL: &[&str] = &[
    "https://shop.app",
    "https://*.shopify.com",
    "https://*.ingest.sentry.io",
    "https://*.ingest.us.sentry.io",
    "https://*.ingest.eu.sentry.io",
];

/// Frame sources needed regardless of GPC (Shopify Shop Pay, Turnstile).
const FRAME_SRC_ESSENTIAL: &[&str] = &[
    "https://cdn.shopify.com",
    "https://shop.app",
    "https://challenges.cloudflare.com",
];

// =============================================================================
// Analytics-only external domains (suppressed when GPC is active)
// =============================================================================

/// Script sources for analytics platforms.
const SCRIPT_SRC_ANALYTICS: &[&str] = &[
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
];

/// Image sources for tracking pixels.
const IMG_SRC_ANALYTICS: &[&str] = &[
    "https://www.facebook.com",
    "https://www.google-analytics.com",
    "https://googleads.g.doubleclick.net",
    "https://ct.pinterest.com",
    "https://t.co",
    "https://analytics.twitter.com",
    "https://tr.snapchat.com",
    "https://analytics.tiktok.com",
    "https://bat.bing.com",
    "https://script.crazyegg.com",
];

/// Connect sources for analytics beacons.
const CONNECT_SRC_ANALYTICS: &[&str] = &[
    "https://www.google-analytics.com",
    "https://analytics.google.com",
    "https://region1.google-analytics.com",
    "https://www.facebook.com",
    "https://connect.facebook.net",
    "https://analytics.tiktok.com",
    "https://googleads.g.doubleclick.net",
    "https://tr.snapchat.com",
    "https://ct.pinterest.com",
    "https://bat.bing.com",
    "https://analytics.twitter.com",
    "https://script.crazyegg.com",
    "https://cloudflareinsights.com",
    "https://api-js.mixpanel.com",
];

/// Frame sources for analytics widgets.
const FRAME_SRC_ANALYTICS: &[&str] = &["https://ct.pinterest.com"];

// =============================================================================
// Middleware
// =============================================================================

/// Add security headers to all responses.
///
/// Headers applied:
/// - `X-Frame-Options: DENY` - Prevent clickjacking
/// - `X-Content-Type-Options: nosniff` - Prevent MIME sniffing
/// - `Referrer-Policy: no-referrer` - Zero referrer leakage
/// - `Content-Security-Policy` - Dynamic CSP with nonce and domain allowlist
/// - `Permissions-Policy` - Deny all sensitive features
/// - `Cache-Control: no-store, max-age=0` - Prevent caching sensitive data
/// - `Cross-Origin-Opener-Policy: same-origin` - Process isolation
/// - `Cross-Origin-Resource-Policy: same-origin` - Resource isolation
/// - `Cross-Origin-Embedder-Policy: credentialless` - Allow CORS resources
/// - `X-DNS-Prefetch-Control: off` - Prevent DNS prefetch leakage
/// - `Sec-GPC: 1` - Echo GPC acknowledgment when browser signal is present
pub async fn security_headers_middleware(request: Request, next: Next) -> Response {
    // Extract nonce BEFORE running the handler (it's set by csp_nonce_middleware)
    let nonce = request
        .extensions()
        .get::<CspNonce>()
        .map(|n| n.value().to_string())
        .unwrap_or_default();

    // Detect Global Privacy Control signal before consuming the request
    let gpc = request
        .headers()
        .get("sec-gpc")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == "1");

    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    // Prevent clickjacking
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));

    // Prevent MIME sniffing
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));

    // Zero referrer leakage (stricter than same-origin)
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));

    // Dynamic CSP with nonce — excludes analytics domains when GPC is active
    let csp = build_csp(&nonce, gpc);
    if let Ok(value) = HeaderValue::from_str(&csp) {
        headers.insert(CONTENT_SECURITY_POLICY, value);
    }

    // Echo GPC acknowledgment (GPC spec Section 4.1)
    if gpc {
        headers.insert(
            HeaderName::from_static("sec-gpc"),
            HeaderValue::from_static("1"),
        );
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

    // HSTS: Enforce HTTPS for 1 year, including subdomains
    // Browsers ignore this header on HTTP connections, so it's safe to always include
    headers.insert(
        HeaderName::from_static("strict-transport-security"),
        HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
    );

    response
}

/// Build the Content-Security-Policy header value.
///
/// When `gpc` is true, analytics domains are excluded to tighten the policy
/// and match the suppressed tracking scripts.
fn build_csp(nonce: &str, gpc: bool) -> String {
    let script_src = join_domains(
        SCRIPT_SRC_ESSENTIAL,
        if gpc { &[] } else { SCRIPT_SRC_ANALYTICS },
    );
    let img_src = join_domains(IMG_SRC_ESSENTIAL, if gpc { &[] } else { IMG_SRC_ANALYTICS });
    let connect_src = join_domains(
        CONNECT_SRC_ESSENTIAL,
        if gpc { &[] } else { CONNECT_SRC_ANALYTICS },
    );
    let frame_src = join_domains(
        FRAME_SRC_ESSENTIAL,
        if gpc { &[] } else { FRAME_SRC_ANALYTICS },
    );

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

/// Join essential and optional domain lists into a single space-separated string.
fn join_domains(essential: &[&str], extra: &[&str]) -> String {
    let capacity = essential.len() + extra.len();
    let mut domains = Vec::with_capacity(capacity);
    domains.extend_from_slice(essential);
    domains.extend_from_slice(extra);
    domains.join(" ")
}

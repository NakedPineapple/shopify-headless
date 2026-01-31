//! Rate limiting middleware using governor and `tower_governor`.
//!
//! Provides configurable rate limiters for different endpoint categories:
//! - `auth_rate_limiter`: Strict limits for authentication endpoints (~10/min)
//! - `api_rate_limiter`: Relaxed limits for general API endpoints (~100/min)

use std::net::IpAddr;
use std::sync::Arc;

use axum::http::Request;
use governor::clock::QuantaInstant;
use governor::middleware::NoOpMiddleware;
use tower_governor::{GovernorError, GovernorLayer, governor::GovernorConfigBuilder};

// =============================================================================
// Custom IP Key Extractor for Cloudflare + Fly.io
// =============================================================================

/// Custom key extractor that checks Cloudflare's `CF-Connecting-IP` header first,
/// then falls back to standard proxy headers.
#[derive(Clone, Copy)]
pub struct CloudflareIpKeyExtractor;

impl tower_governor::key_extractor::KeyExtractor for CloudflareIpKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        let headers = req.headers();

        // Try CF-Connecting-IP first (Cloudflare's real client IP)
        if let Some(ip) = headers
            .get("cf-connecting-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<IpAddr>().ok())
        {
            return Ok(ip);
        }

        // Try X-Forwarded-For (first IP in the chain)
        if let Some(ip) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.trim().parse::<IpAddr>().ok())
        {
            return Ok(ip);
        }

        // Try X-Real-IP
        if let Some(ip) = headers
            .get("x-real-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<IpAddr>().ok())
        {
            return Ok(ip);
        }

        // Try Fly-Client-IP (Fly.io's header)
        if let Some(ip) = headers
            .get("fly-client-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<IpAddr>().ok())
        {
            return Ok(ip);
        }

        Err(GovernorError::UnableToExtractKey)
    }
}

// =============================================================================
// Rate Limiter Configuration
// =============================================================================

/// Rate limiter layer type for Axum.
///
/// Uses `CloudflareIpKeyExtractor` to get the real client IP from Cloudflare
/// and Fly.io proxy headers.
pub type RateLimiterLayer =
    GovernorLayer<CloudflareIpKeyExtractor, NoOpMiddleware<QuantaInstant>, axum::body::Body>;

/// Create rate limiter for auth endpoints: ~10 requests per minute per IP.
///
/// Configuration: 1 request every 6 seconds (replenish), burst of 5.
/// This prevents brute force attacks on login/registration endpoints.
///
/// # Panics
///
/// This function will not panic. The configuration uses only valid positive
/// integers (`per_second(6)` and `burst_size(5)`), which are always accepted
/// by `GovernorConfigBuilder`.
#[must_use]
pub fn auth_rate_limiter() -> RateLimiterLayer {
    let config = GovernorConfigBuilder::default()
        .key_extractor(CloudflareIpKeyExtractor)
        .per_second(6) // Replenish 1 token every 6 seconds (~10/minute)
        .burst_size(5) // Allow burst of 5 requests
        .finish()
        .expect("rate limiter config with per_second(6) and burst_size(5) is valid");
    GovernorLayer::new(Arc::new(config))
}

/// Create rate limiter for general API: ~100 requests per minute per IP.
///
/// Configuration: 1 request per second (replenish), burst of 50.
/// This prevents abuse of cart and other API endpoints.
///
/// # Panics
///
/// This function will not panic. The configuration uses only valid positive
/// integers (`per_second(1)` and `burst_size(50)`), which are always accepted
/// by `GovernorConfigBuilder`.
#[must_use]
pub fn api_rate_limiter() -> RateLimiterLayer {
    let config = GovernorConfigBuilder::default()
        .key_extractor(CloudflareIpKeyExtractor)
        .per_second(1) // Replenish quickly
        .burst_size(50) // Allow burst of 50 requests
        .finish()
        .expect("rate limiter config with per_second(1) and burst_size(50) is valid");
    GovernorLayer::new(Arc::new(config))
}

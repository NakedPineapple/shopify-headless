//! Rate limiting middleware using governor and `tower_governor`.
//!
//! Provides configurable rate limiters for different endpoint categories:
//! - `auth_rate_limiter`: Strict limits for authentication endpoints (~10/min)
//! - `api_rate_limiter`: Relaxed limits for general API endpoints (~100/min)

use governor::clock::QuantaInstant;
use governor::middleware::NoOpMiddleware;
use std::sync::Arc;
use tower_governor::{
    GovernorLayer, governor::GovernorConfigBuilder, key_extractor::PeerIpKeyExtractor,
};

/// Rate limiter layer type for Axum with default configuration.
pub type RateLimiterLayer =
    GovernorLayer<PeerIpKeyExtractor, NoOpMiddleware<QuantaInstant>, axum::body::Body>;

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
        .per_second(1) // Replenish quickly
        .burst_size(50) // Allow burst of 50 requests
        .finish()
        .expect("rate limiter config with per_second(1) and burst_size(50) is valid");
    GovernorLayer::new(Arc::new(config))
}

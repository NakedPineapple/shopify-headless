//! Per-request site context resolved from the `Host` header.
//!
//! Provides domain-specific values (base URL, Cloudflare beacon token) for
//! multi-domain deployments where a single storefront serves multiple hostnames.

use axum::{extract::FromRequestParts, http::request::Parts};

use crate::state::AppState;

/// Per-request site context resolved from the `Host` header.
///
/// Contains domain-specific values that vary depending on which hostname
/// the user is visiting (e.g., `nakedpineapple.co` vs `pineappleskinco.com`).
///
/// # Example
///
/// ```ignore
/// async fn handler(site: SiteContext) -> impl IntoResponse {
///     MyTemplate { site, /* ... */ }
/// }
/// ```
#[derive(Clone, Debug)]
pub struct SiteContext {
    /// The hostname from the request (e.g., `nakedpineapple.co`), without port.
    pub host: String,
    /// Full base URL for this domain (e.g., `https://nakedpineapple.co`)
    pub base_url: String,
    /// Cloudflare Web Analytics beacon token (empty if not configured)
    pub cf_beacon_token: String,
}

impl FromRequestParts<AppState> for SiteContext {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let host = parts
            .headers
            .get(axum::http::header::HOST)
            .and_then(|h| h.to_str().ok())
            .map_or("", |h| h.split(':').next().unwrap_or(h));

        let config = state.config();

        let base_url = if host.is_empty() {
            tracing::warn!("Host header missing from request — using default base URL");
            config.default_base_url.clone()
        } else {
            config
                .base_urls
                .get(host)
                .cloned()
                .unwrap_or_else(|| config.default_base_url.clone())
        };

        let cf_beacon_token = config
            .cf_beacon_tokens
            .get(host)
            .cloned()
            .unwrap_or_default();

        Ok(Self {
            host: host.to_owned(),
            base_url,
            cf_beacon_token,
        })
    }
}

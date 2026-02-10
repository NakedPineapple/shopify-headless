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
    /// Whether the browser sent the `Sec-GPC: 1` Global Privacy Control signal.
    pub gpc: bool,
    /// Whether AI chat support is fully configured and enabled.
    pub chat_enabled: bool,
    /// Cloudflare Turnstile public site key (empty if not configured).
    pub turnstile_site_key: String,
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
            config.origin_for(&config.default_host)
        } else if config.hosts.contains(host) {
            config.origin_for(host)
        } else {
            config.origin_for(&config.default_host)
        };

        let cf_beacon_token = config
            .cf_beacon_tokens
            .get(host)
            .cloned()
            .unwrap_or_default();

        let gpc = parts
            .headers
            .get("sec-gpc")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v == "1");

        let chat_enabled = state.is_chat_enabled();
        let turnstile_site_key = config.turnstile_site_key.clone().unwrap_or_default();

        Ok(Self {
            host: host.to_owned(),
            base_url,
            cf_beacon_token,
            gpc,
            chat_enabled,
            turnstile_site_key,
        })
    }
}

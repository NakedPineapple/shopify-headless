//! CSP nonce middleware for inline script protection.
//!
//! Generates a unique, cryptographically random nonce per request.
//! Include this in `<script nonce="...">` tags and the CSP header.

use axum::{
    extract::{FromRequestParts, Request},
    http::request::Parts,
    middleware::Next,
    response::Response,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use rand::RngCore;

/// A CSP nonce value for inline scripts.
///
/// Each request gets a unique, cryptographically random nonce (128-bit, base64-encoded).
#[derive(Clone, Debug)]
pub struct CspNonce(pub String);

impl CspNonce {
    /// Generate a new random nonce.
    #[must_use]
    pub fn generate() -> Self {
        let mut bytes = [0u8; 16];
        rand::rng().fill_bytes(&mut bytes);
        Self(STANDARD.encode(bytes))
    }

    /// Get the nonce value for use in templates.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.0
    }
}

/// Middleware that generates a CSP nonce and stores it in request extensions.
///
/// Must be added before `security_headers_middleware` in the middleware stack
/// so the nonce is available when building the CSP header.
pub async fn csp_nonce_middleware(mut request: Request, next: Next) -> Response {
    request.extensions_mut().insert(CspNonce::generate());
    next.run(request).await
}

/// Extractor to get the CSP nonce from request extensions.
///
/// # Example
///
/// ```ignore
/// async fn handler(CspNonce(nonce): CspNonce) -> impl IntoResponse {
///     MyTemplate { nonce, /* ... */ }
/// }
/// ```
impl<S> FromRequestParts<S> for CspNonce
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.extensions.get::<Self>().cloned().unwrap_or_else(|| {
            tracing::warn!(
                "CSP nonce not found in request extensions - middleware may be misconfigured"
            );
            Self(String::new())
        }))
    }
}

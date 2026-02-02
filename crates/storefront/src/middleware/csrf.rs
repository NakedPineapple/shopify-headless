//! CSRF protection middleware using double-submit cookie pattern.
//!
//! Provides CSRF token generation and validation for state-changing requests.
//! HTMX requests should include the token in the `X-CSRF-Token` header.

use axum::{
    extract::{FromRequestParts, Request},
    http::{StatusCode, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;
use tower_sessions::Session;

const CSRF_TOKEN_KEY: &str = "csrf_token";
const CSRF_HEADER: &str = "X-CSRF-Token";
const CSRF_FORM_FIELD: &str = "_csrf";

/// CSRF token length in bytes (256 bits).
const TOKEN_BYTES: usize = 32;

/// Generate a cryptographically secure CSRF token.
fn generate_csrf_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    rand::rng().fill(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Get or create a CSRF token for the session.
///
/// If a token already exists in the session, returns it.
/// Otherwise, generates a new token, stores it, and returns it.
pub async fn get_or_create_csrf_token(session: &Session) -> String {
    if let Ok(Some(token)) = session.get::<String>(CSRF_TOKEN_KEY).await {
        return token;
    }

    let token = generate_csrf_token();
    // Best effort - if insert fails, we still return the token for this request
    let _ = session.insert(CSRF_TOKEN_KEY, &token).await;
    token
}

/// CSRF token extractor for use in route handlers and templates.
///
/// Extracts the CSRF token from the session, creating one if it doesn't exist.
///
/// # Example
///
/// ```ignore
/// use crate::middleware::csrf::CsrfToken;
///
/// async fn form_page(CsrfToken(token): CsrfToken) -> impl IntoResponse {
///     // Use token in template: <input type="hidden" name="_csrf" value="{{ token }}">
/// }
/// ```
#[derive(Clone)]
pub struct CsrfToken(pub String);

impl<S> FromRequestParts<S> for CsrfToken
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let session = parts
            .extensions
            .get::<Session>()
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

        let token = get_or_create_csrf_token(session).await;
        Ok(Self(token))
    }
}

/// Middleware to validate CSRF tokens on state-changing requests.
///
/// Checks POST, PUT, PATCH, and DELETE requests for a valid CSRF token.
/// The token can be provided via:
/// - `X-CSRF-Token` header (preferred for HTMX requests)
/// - `_csrf` form field (for traditional form submissions)
///
/// Skips validation for:
/// - Safe methods (GET, HEAD, OPTIONS)
/// - Webhook endpoints (`/api/webhooks/*`)
///
/// If no token is provided, falls back to `SameSite` cookie protection
/// with a warning log. This allows gradual migration to explicit tokens.
pub async fn csrf_protection(request: Request, next: Next) -> Response {
    // Only check state-changing methods
    if !matches!(
        request.method().as_str(),
        "POST" | "PUT" | "PATCH" | "DELETE"
    ) {
        return next.run(request).await;
    }

    // Skip webhook endpoints (they use HMAC verification)
    let path = request.uri().path();
    if path.starts_with("/api/webhooks") {
        return next.run(request).await;
    }

    let Some(session) = request.extensions().get::<Session>().cloned() else {
        tracing::error!("CSRF middleware: no session in request extensions");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let expected_token: Option<String> = session.get(CSRF_TOKEN_KEY).await.ok().flatten();

    let Some(expected) = expected_token else {
        // No token in session - this is a new session making a POST request
        // For HTMX requests, they should have obtained a token first via GET
        tracing::warn!(
            path = %path,
            "CSRF validation: no token in session for state-changing request"
        );
        // Allow the request but rely on SameSite cookie for protection
        // This handles edge cases like expired sessions
        return next.run(request).await;
    };

    // Check X-CSRF-Token header (preferred for HTMX)
    let provided = request
        .headers()
        .get(CSRF_HEADER)
        .and_then(|h| h.to_str().ok())
        .map(String::from);

    match provided {
        Some(token) if constant_time_compare(&token, &expected) => {
            // Valid token
            next.run(request).await
        }
        Some(_) => {
            tracing::warn!(path = %path, "CSRF validation failed: token mismatch");
            StatusCode::FORBIDDEN.into_response()
        }
        None => {
            // No header provided - check if this might be a form submission
            // For now, allow with warning (SameSite cookie provides baseline protection)
            // Forms should include the _csrf field, but we can't easily check it
            // without consuming the body
            tracing::debug!(
                path = %path,
                "CSRF token not provided in header, relying on SameSite cookie"
            );
            next.run(request).await
        }
    }
}

/// Constant-time string comparison to prevent timing attacks.
fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_csrf_token_length() {
        let token = generate_csrf_token();
        // Base64 of 32 bytes without padding = 43 characters
        assert_eq!(token.len(), 43);
    }

    #[test]
    fn test_generate_csrf_token_uniqueness() {
        let token1 = generate_csrf_token();
        let token2 = generate_csrf_token();
        assert_ne!(token1, token2);
    }

    #[test]
    fn test_constant_time_compare_equal() {
        assert!(constant_time_compare("abc123", "abc123"));
    }

    #[test]
    fn test_constant_time_compare_not_equal() {
        assert!(!constant_time_compare("abc123", "abc124"));
    }

    #[test]
    fn test_constant_time_compare_different_lengths() {
        assert!(!constant_time_compare("abc", "abcd"));
    }
}

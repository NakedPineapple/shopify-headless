//! Cloudflare Turnstile bot verification.
//!
//! Verifies Turnstile tokens server-side to protect AI chat endpoints from bots.
//! Only used at conversation creation — subsequent messages don't re-verify.

use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

/// Cloudflare Turnstile verification endpoint.
const SITEVERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

/// Error returned when Turnstile verification fails.
#[derive(Debug, thiserror::Error)]
pub enum TurnstileError {
    #[error("missing turnstile token")]
    MissingToken,
    #[error("verification request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("turnstile verification failed")]
    VerificationFailed,
}

/// Response from Cloudflare Turnstile siteverify API.
#[derive(Deserialize)]
struct SiteVerifyResponse {
    success: bool,
}

/// Verify a Cloudflare Turnstile token.
///
/// # Errors
///
/// Returns `TurnstileError` if the token is missing, the request fails,
/// or verification is unsuccessful.
pub async fn verify_turnstile_token(
    secret_key: &SecretString,
    token: &str,
) -> Result<(), TurnstileError> {
    if token.is_empty() {
        return Err(TurnstileError::MissingToken);
    }

    let client = reqwest::Client::new();
    let response: SiteVerifyResponse = client
        .post(SITEVERIFY_URL)
        .form(&[("secret", secret_key.expose_secret()), ("response", token)])
        .send()
        .await?
        .json()
        .await?;

    if response.success {
        Ok(())
    } else {
        Err(TurnstileError::VerificationFailed)
    }
}

//! `ShipHero` authentication module.
//!
//! Handles email/password authentication to obtain JWT tokens for API access.

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::ShipHeroError;

/// `ShipHero` authentication endpoint.
const AUTH_ENDPOINT: &str = "https://public-api.shiphero.com/auth/token";

/// JWT token obtained from `ShipHero` authentication.
#[derive(Debug, Clone)]
pub struct ShipHeroToken {
    /// JWT access token for API requests.
    pub access_token: SecretString,
    /// Optional refresh token for obtaining new access tokens.
    pub refresh_token: Option<SecretString>,
    /// Unix timestamp when the access token expires.
    pub access_token_expires_at: i64,
    /// Unix timestamp when the refresh token expires (if applicable).
    pub refresh_token_expires_at: Option<i64>,
}

/// Request body for `ShipHero` authentication.
#[derive(Serialize)]
struct AuthRequest<'a> {
    email: &'a str,
    password: &'a str,
}

/// Response from `ShipHero` authentication endpoint.
#[derive(Deserialize)]
struct AuthResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    /// Token lifetime in seconds.
    expires_in: i64,
    /// Refresh token lifetime in seconds (if applicable).
    #[serde(default)]
    refresh_expires_in: Option<i64>,
}

/// Error response from `ShipHero` authentication endpoint.
#[derive(Deserialize)]
struct AuthErrorResponse {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

/// Authenticate with `ShipHero` using email and password.
///
/// Returns a JWT token that can be used for API requests.
///
/// # Arguments
///
/// * `email` - `ShipHero` account email
/// * `password` - `ShipHero` account password
///
/// # Errors
///
/// Returns `ShipHeroError::AuthenticationFailed` if credentials are invalid.
#[instrument(skip(password), fields(email = %email))]
pub async fn authenticate(
    client: &reqwest::Client,
    email: &str,
    password: &SecretString,
) -> Result<ShipHeroToken, ShipHeroError> {
    let now = chrono::Utc::now().timestamp();

    let response = client
        .post(AUTH_ENDPOINT)
        .json(&AuthRequest {
            email,
            password: password.expose_secret(),
        })
        .send()
        .await?;

    let status = response.status();

    if status.is_success() {
        let auth_response: AuthResponse = response.json().await?;

        Ok(ShipHeroToken {
            access_token: SecretString::from(auth_response.access_token),
            refresh_token: auth_response.refresh_token.map(SecretString::from),
            access_token_expires_at: now + auth_response.expires_in,
            refresh_token_expires_at: auth_response.refresh_expires_in.map(|secs| now + secs),
        })
    } else if status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
    {
        let error_response: AuthErrorResponse =
            response.json().await.unwrap_or_else(|_| AuthErrorResponse {
                error: None,
                message: Some("Invalid credentials".to_string()),
            });

        let message = error_response
            .message
            .or(error_response.error)
            .unwrap_or_else(|| "Invalid credentials".to_string());

        Err(ShipHeroError::AuthenticationFailed(message))
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        Err(ShipHeroError::AuthenticationFailed(format!(
            "HTTP {status}: {error_text}"
        )))
    }
}

/// Refresh an access token using a refresh token.
///
/// # Arguments
///
/// * `refresh_token` - The refresh token from a previous authentication
///
/// # Errors
///
/// Returns `ShipHeroError::AuthenticationFailed` if the refresh token is invalid or expired.
#[instrument(skip(refresh_token))]
pub async fn refresh_access_token(
    client: &reqwest::Client,
    refresh_token: &SecretString,
) -> Result<ShipHeroToken, ShipHeroError> {
    let now = chrono::Utc::now().timestamp();

    let response = client
        .post(AUTH_ENDPOINT)
        .json(&serde_json::json!({
            "refresh_token": refresh_token.expose_secret()
        }))
        .send()
        .await?;

    let status = response.status();

    if status.is_success() {
        let auth_response: AuthResponse = response.json().await?;

        Ok(ShipHeroToken {
            access_token: SecretString::from(auth_response.access_token),
            refresh_token: auth_response.refresh_token.map(SecretString::from),
            access_token_expires_at: now + auth_response.expires_in,
            refresh_token_expires_at: auth_response.refresh_expires_in.map(|secs| now + secs),
        })
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        Err(ShipHeroError::AuthenticationFailed(format!(
            "Token refresh failed: {error_text}"
        )))
    }
}

impl ShipHeroToken {
    /// Check if the access token has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        // Consider expired if less than 60 seconds remaining
        now >= self.access_token_expires_at - 60
    }

    /// Check if the access token will expire within the given number of seconds.
    #[must_use]
    pub fn expires_within(&self, seconds: i64) -> bool {
        let now = chrono::Utc::now().timestamp();
        now >= self.access_token_expires_at - seconds
    }

    /// Check if a refresh token is available and not expired.
    #[must_use]
    pub fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
            && self.refresh_token_expires_at.is_none_or(|expires_at| {
                let now = chrono::Utc::now().timestamp();
                now < expires_at - 60
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_is_expired() {
        let now = chrono::Utc::now().timestamp();

        // Token that expired an hour ago
        let expired_token = ShipHeroToken {
            access_token: SecretString::from("test"),
            refresh_token: None,
            access_token_expires_at: now - 3600,
            refresh_token_expires_at: None,
        };
        assert!(expired_token.is_expired());

        // Token that expires in an hour
        let valid_token = ShipHeroToken {
            access_token: SecretString::from("test"),
            refresh_token: None,
            access_token_expires_at: now + 3600,
            refresh_token_expires_at: None,
        };
        assert!(!valid_token.is_expired());

        // Token that expires in 30 seconds (should be considered expired due to 60s buffer)
        let almost_expired_token = ShipHeroToken {
            access_token: SecretString::from("test"),
            refresh_token: None,
            access_token_expires_at: now + 30,
            refresh_token_expires_at: None,
        };
        assert!(almost_expired_token.is_expired());
    }

    #[test]
    fn test_can_refresh() {
        let now = chrono::Utc::now().timestamp();

        // Token with valid refresh token
        let with_refresh = ShipHeroToken {
            access_token: SecretString::from("test"),
            refresh_token: Some(SecretString::from("refresh")),
            access_token_expires_at: now - 3600, // expired
            refresh_token_expires_at: Some(now + 86400), // refresh valid for a day
        };
        assert!(with_refresh.can_refresh());

        // Token without refresh token
        let without_refresh = ShipHeroToken {
            access_token: SecretString::from("test"),
            refresh_token: None,
            access_token_expires_at: now - 3600,
            refresh_token_expires_at: None,
        };
        assert!(!without_refresh.can_refresh());

        // Token with expired refresh token
        let expired_refresh = ShipHeroToken {
            access_token: SecretString::from("test"),
            refresh_token: Some(SecretString::from("refresh")),
            access_token_expires_at: now - 3600,
            refresh_token_expires_at: Some(now - 1800), // expired
        };
        assert!(!expired_refresh.can_refresh());
    }
}

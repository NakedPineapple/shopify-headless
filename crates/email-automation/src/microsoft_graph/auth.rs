//! `OAuth2` token acquisition and caching for Microsoft Graph API.
//!
//! Uses the client credentials flow (application permissions) to authenticate
//! with Azure AD and obtain access tokens for the Graph API.

use chrono::{Duration, Utc};
use secrecy::ExposeSecret;
use tokio::sync::RwLock;

use super::error::M365Error;
use super::types::{AccessToken, TokenResponse};
use crate::config::M365Config;

/// Buffer before token expiry to trigger a refresh (5 minutes).
const REFRESH_BUFFER_MINUTES: i64 = 5;

/// Manages `OAuth2` token lifecycle for Microsoft Graph API.
pub struct TokenManager {
    config: M365Config,
    http: reqwest::Client,
    token: RwLock<Option<AccessToken>>,
}

impl TokenManager {
    /// Create a new token manager.
    pub fn new(config: &M365Config, http: &reqwest::Client) -> Self {
        Self {
            config: config.clone(),
            http: http.clone(),
            token: RwLock::new(None),
        }
    }

    /// Get a valid access token, refreshing if necessary.
    ///
    /// # Errors
    ///
    /// Returns `M365Error::Authentication` if token acquisition fails.
    pub async fn get_token(&self) -> Result<String, M365Error> {
        // Fast path: check if we have a valid token
        {
            let guard = self.token.read().await;
            if let Some(token) = &*guard
                && !token.is_expired_with_buffer(Duration::minutes(REFRESH_BUFFER_MINUTES))
            {
                return Ok(token.token.clone());
            }
        }

        // Slow path: acquire a new token
        self.refresh_token().await
    }

    /// Force a token refresh.
    async fn refresh_token(&self) -> Result<String, M365Error> {
        let mut guard = self.token.write().await;

        // Double-check after acquiring the write lock (another task may have refreshed)
        if let Some(token) = &*guard
            && !token.is_expired_with_buffer(Duration::minutes(REFRESH_BUFFER_MINUTES))
        {
            return Ok(token.token.clone());
        }

        let token_url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.config.tenant_id
        );

        let response = self
            .http
            .post(&token_url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", &self.config.client_id),
                ("client_secret", self.config.client_secret.expose_secret()),
                ("scope", "https://graph.microsoft.com/.default"),
            ])
            .send()
            .await
            .map_err(|e| M365Error::Authentication(format!("token request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "no response body".to_string());
            return Err(M365Error::Authentication(format!(
                "token endpoint returned HTTP {status}: {body}"
            )));
        }

        let token_response: TokenResponse = response.json().await.map_err(|e| {
            M365Error::Authentication(format!("failed to parse token response: {e}"))
        })?;

        let access_token = AccessToken {
            token: token_response.access_token.clone(),
            expires_at: Utc::now() + Duration::seconds(token_response.expires_in),
        };

        tracing::info!(
            token_type = %token_response.token_type,
            expires_in_secs = token_response.expires_in,
            "acquired Microsoft Graph access token"
        );

        let token_str = access_token.token.clone();
        *guard = Some(access_token);
        drop(guard);

        Ok(token_str)
    }
}

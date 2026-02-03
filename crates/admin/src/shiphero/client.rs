//! `ShipHero` GraphQL API client.
//!
//! Provides type-safe access to the `ShipHero` API for viewing warehouse data
//! including orders, shipments, and inventory.

use std::sync::Arc;

use graphql_client::GraphQLQuery;
use secrecy::ExposeSecret;
use serde::{Deserialize, de::DeserializeOwned};
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

use super::auth::{ShipHeroToken, authenticate, refresh_access_token};
use super::{GraphQLError, GraphQLErrorLocation, ShipHeroError};

/// `ShipHero` GraphQL API endpoint.
const GRAPHQL_ENDPOINT: &str = "https://public-api.shiphero.com/graphql";

/// `ShipHero` GraphQL API client.
///
/// Provides read-only access to warehouse data including orders awaiting
/// fulfillment, shipment history, and inventory levels.
///
/// # Authentication
///
/// Uses JWT tokens obtained from email/password authentication. Tokens are
/// cached in memory and refreshed automatically when expired.
#[derive(Clone)]
pub struct ShipHeroClient {
    inner: Arc<ShipHeroClientInner>,
}

struct ShipHeroClientInner {
    client: reqwest::Client,
    /// In-memory token cache
    token: RwLock<Option<ShipHeroToken>>,
}

/// GraphQL response wrapper.
#[derive(Debug, Deserialize)]
struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLErrorResponse>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLErrorResponse {
    message: String,
    #[serde(default)]
    locations: Vec<GraphQLErrorLocationResponse>,
    #[serde(default)]
    path: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct GraphQLErrorLocationResponse {
    line: i64,
    column: i64,
}

/// Connection status for `ShipHero`.
#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    /// Not connected - no credentials stored.
    NotConnected,
    /// Connected with valid token.
    Connected {
        /// Email used to authenticate.
        email: String,
        /// Unix timestamp when the access token expires.
        expires_at: i64,
    },
    /// Token has expired - re-authentication required.
    TokenExpired {
        /// Email used to authenticate.
        email: String,
    },
}

impl ShipHeroClient {
    /// Create a new `ShipHero` API client without a token.
    ///
    /// Use `set_token` to set the authentication token, or
    /// `authenticate` to obtain a new token.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created. This should never happen
    /// under normal circumstances as we use standard TLS configuration.
    #[must_use]
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            inner: Arc::new(ShipHeroClientInner {
                client,
                token: RwLock::new(None),
            }),
        }
    }

    /// Create a new client with an existing token.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created (should never happen in practice).
    #[must_use]
    pub fn with_token(token: ShipHeroToken) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            inner: Arc::new(ShipHeroClientInner {
                client,
                token: RwLock::new(Some(token)),
            }),
        }
    }

    // =========================================================================
    // Authentication
    // =========================================================================

    /// Authenticate with `ShipHero` using email and password.
    ///
    /// Stores the obtained JWT token in the client for subsequent API calls.
    ///
    /// # Arguments
    ///
    /// * `email` - `ShipHero` account email
    /// * `password` - `ShipHero` account password
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError::AuthenticationFailed` if credentials are invalid.
    #[instrument(skip(self, password), fields(email = %email))]
    pub async fn authenticate(
        &self,
        email: &str,
        password: &secrecy::SecretString,
    ) -> Result<ShipHeroToken, ShipHeroError> {
        debug!("Authenticating with ShipHero");

        let token = authenticate(&self.inner.client, email, password).await?;

        // Cache the token
        *self.inner.token.write().await = Some(token.clone());

        info!("ShipHero authentication successful");
        Ok(token)
    }

    /// Set the access token directly (for loading from storage).
    #[instrument(skip(self, token))]
    pub async fn set_token(&self, token: ShipHeroToken) {
        debug!(
            expires_at = token.access_token_expires_at,
            "Setting ShipHero token from storage"
        );
        *self.inner.token.write().await = Some(token);
    }

    /// Get the current token (if set).
    pub async fn get_token(&self) -> Option<ShipHeroToken> {
        self.inner.token.read().await.clone()
    }

    /// Check if we have a valid (non-expired) token.
    pub async fn has_valid_token(&self) -> bool {
        self.inner
            .token
            .read()
            .await
            .as_ref()
            .is_some_and(|token| !token.is_expired())
    }

    /// Clear the cached token.
    #[instrument(skip(self))]
    pub async fn clear_token(&self) {
        debug!("Clearing cached ShipHero token");
        *self.inner.token.write().await = None;
    }

    /// Attempt to refresh the access token if it's expired.
    ///
    /// Returns `Ok(())` if the token was refreshed or is still valid.
    /// Returns an error if refresh fails or no refresh token is available.
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError::TokenExpired` if the token is expired and cannot be refreshed.
    /// Returns `ShipHeroError::NoAccessToken` if no token is set.
    /// Returns `ShipHeroError::AuthenticationFailed` if the refresh token is rejected.
    #[instrument(skip(self))]
    pub async fn try_refresh_token(&self) -> Result<(), ShipHeroError> {
        debug!("Attempting to refresh ShipHero token");
        let token = self.inner.token.read().await.clone();

        if let Some(token) = token {
            if token.is_expired() {
                if token.can_refresh()
                    && let Some(ref refresh_token) = token.refresh_token
                {
                    debug!("Token expired, refreshing using refresh token");
                    let new_token = refresh_access_token(&self.inner.client, refresh_token).await?;
                    *self.inner.token.write().await = Some(new_token);
                    info!("ShipHero token refreshed successfully");
                    return Ok(());
                }
                warn!("ShipHero token expired and cannot be refreshed");
                return Err(ShipHeroError::TokenExpired);
            }
            debug!("ShipHero token still valid, no refresh needed");
            Ok(())
        } else {
            warn!("No ShipHero token available to refresh");
            Err(ShipHeroError::NoAccessToken)
        }
    }

    // =========================================================================
    // GraphQL Execution
    // =========================================================================

    /// Execute a GraphQL query.
    ///
    /// Automatically handles token refresh if the token is close to expiring.
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError::NoAccessToken` if no token is set.
    /// Returns `ShipHeroError::TokenExpired` if the token is expired.
    /// Returns `ShipHeroError::RateLimited` if we're being rate limited.
    /// Returns `ShipHeroError::GraphQL` if the query returns errors.
    /// Returns `ShipHeroError::Http` on network failures.
    #[instrument(skip(self, query, variables, operation_name))]
    pub async fn execute<T: DeserializeOwned>(
        &self,
        query: &str,
        variables: Option<serde_json::Value>,
        operation_name: Option<&str>,
    ) -> Result<T, ShipHeroError> {
        debug!("Executing ShipHero GraphQL query");
        let start = std::time::Instant::now();

        // Try to refresh token if it's close to expiring
        if let Some(token) = self.inner.token.read().await.as_ref()
            && token.expires_within(300)
            && token.can_refresh()
        {
            // Token expires within 5 minutes, try to refresh
            debug!("Token expiring soon, attempting proactive refresh");
            let _ = self.try_refresh_token().await;
        }

        let access_token = self.get_access_token().await?;

        let body = serde_json::json!({
            "query": query,
            "variables": variables.unwrap_or(serde_json::Value::Null),
            "operationName": operation_name,
        });

        let response = self
            .inner
            .client
            .post(GRAPHQL_ENDPOINT)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();

        // Check for rate limiting
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(60);
            warn!(
                retry_after_secs = retry_after,
                duration_ms = %start.elapsed().as_millis(),
                "Rate limited by ShipHero API"
            );
            return Err(ShipHeroError::RateLimited(retry_after));
        }

        // Check for unauthorized
        if status == reqwest::StatusCode::UNAUTHORIZED {
            warn!(
                duration_ms = %start.elapsed().as_millis(),
                "ShipHero API returned unauthorized - token may be expired"
            );
            return Err(ShipHeroError::TokenExpired);
        }

        let response_text = response.text().await?;
        debug!(
            response_bytes = response_text.len(),
            response_preview = %truncate_for_log(&response_text, 2000),
            "Raw ShipHero GraphQL response"
        );

        let graphql_response: GraphQLResponse<T> =
            serde_json::from_str(&response_text).map_err(|e| {
                warn!(
                    error = %e,
                    response_preview = %truncate_for_log(&response_text, 500),
                    "Failed to deserialize ShipHero response"
                );
                ShipHeroError::Deserialization(e.to_string())
            })?;

        check_graphql_errors(&graphql_response, status, &start)?;

        debug!(
            status = %status,
            duration_ms = %start.elapsed().as_millis(),
            "ShipHero GraphQL query completed successfully"
        );

        graphql_response.data.ok_or_else(|| {
            warn!("ShipHero GraphQL response contained no data");
            ShipHeroError::GraphQL(vec![GraphQLError {
                message: "No data in response".to_string(),
                locations: vec![],
                path: vec![],
            }])
        })
    }

    /// Execute a typed GraphQL query using `graphql_client`.
    ///
    /// This is the preferred method for executing queries as it provides
    /// compile-time type safety for both variables and responses.
    ///
    /// # Errors
    ///
    /// Returns the same errors as `execute`.
    pub async fn execute_query<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<Q::ResponseData, ShipHeroError>
    where
        Q::Variables: serde::Serialize,
        Q::ResponseData: DeserializeOwned,
    {
        let body = Q::build_query(variables);
        let query = body.query;
        let operation_name = body.operation_name;
        let variables = serde_json::to_value(body.variables)?;

        self.execute(query, Some(variables), Some(operation_name))
            .await
    }

    /// Get the current access token string.
    async fn get_access_token(&self) -> Result<String, ShipHeroError> {
        let token = self.inner.token.read().await;

        if let Some(ref token) = *token {
            if token.is_expired() {
                return Err(ShipHeroError::TokenExpired);
            }
            Ok(token.access_token.expose_secret().to_string())
        } else {
            Err(ShipHeroError::NoAccessToken)
        }
    }

    /// Test the connection by fetching account info.
    ///
    /// Returns basic account information if the connection is valid.
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError` if the connection test fails due to authentication
    /// issues, network errors, or GraphQL errors.
    #[instrument(skip(self))]
    pub async fn test_connection(&self) -> Result<AccountInfo, ShipHeroError> {
        debug!("Testing ShipHero API connection");
        let query = r"
            query {
                account {
                    request_id
                    data {
                        id
                        email
                        username
                    }
                }
            }
        ";

        #[derive(Debug, Deserialize)]
        struct Response {
            account: AccountResult,
        }

        #[derive(Debug, Deserialize)]
        struct AccountResult {
            data: AccountInfo,
        }

        let response: Response = self.execute(query, None, None).await?;
        debug!(
            account_id = %response.account.data.id,
            email = %response.account.data.email,
            "ShipHero connection test successful"
        );
        Ok(response.account.data)
    }
}

impl Default for ShipHeroClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Basic `ShipHero` account information.
#[derive(Debug, Clone, Deserialize)]
pub struct AccountInfo {
    /// Account ID.
    pub id: String,
    /// Account email.
    pub email: String,
    /// Username.
    pub username: Option<String>,
}

/// Check a GraphQL response for errors and convert them to `ShipHeroError`.
fn check_graphql_errors<T>(
    response: &GraphQLResponse<T>,
    status: reqwest::StatusCode,
    start: &std::time::Instant,
) -> Result<(), ShipHeroError> {
    let Some(errors) = response.errors.as_ref().filter(|e| !e.is_empty()) else {
        return Ok(());
    };
    let error_messages: Vec<&str> = errors.iter().map(|e| e.message.as_str()).collect();
    warn!(
        status = %status,
        duration_ms = %start.elapsed().as_millis(),
        errors = ?error_messages,
        "ShipHero GraphQL query returned errors"
    );
    let converted: Vec<GraphQLError> = errors
        .iter()
        .map(|e| GraphQLError {
            message: e.message.clone(),
            locations: e
                .locations
                .iter()
                .map(|l| GraphQLErrorLocation {
                    line: l.line,
                    column: l.column,
                })
                .collect(),
            path: e.path.clone(),
        })
        .collect();
    Err(ShipHeroError::GraphQL(converted))
}

fn truncate_for_log(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len { s } else { &s[..max_len] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = ShipHeroClient::new();
        assert!(client.inner.token.blocking_read().is_none());
    }

    #[test]
    fn test_client_with_token() {
        use secrecy::SecretString;

        let token = ShipHeroToken {
            access_token: SecretString::from("test_token"),
            refresh_token: None,
            access_token_expires_at: chrono::Utc::now().timestamp() + 3600,
            refresh_token_expires_at: None,
        };

        let client = ShipHeroClient::with_token(token);
        assert!(client.inner.token.blocking_read().is_some());
    }
}

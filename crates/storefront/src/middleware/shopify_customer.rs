//! Shopify Customer authentication middleware and extractors.
//!
//! Provides extractors for requiring Shopify Customer authentication in route handlers.

use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;

use crate::models::session_keys;
use crate::shopify::CustomerAccessToken;

/// Extractor that requires Shopify Customer authentication.
///
/// If the customer is not logged in via Shopify OAuth, returns a redirect
/// to the Shopify login page.
///
/// # Example
///
/// ```rust,ignore
/// async fn protected_handler(
///     RequireShopifyCustomer(token): RequireShopifyCustomer,
/// ) -> impl IntoResponse {
///     // Use token.access_token to make Shopify Customer API calls
///     format!("Customer authenticated!")
/// }
/// ```
pub struct RequireShopifyCustomer(pub CustomerAccessToken);

/// Error returned when Shopify Customer authentication is required but not present.
pub enum ShopifyCustomerRejection {
    /// Redirect to Shopify login page (for HTML requests).
    RedirectToLogin,
    /// Unauthorized response (for API requests).
    Unauthorized,
}

impl IntoResponse for ShopifyCustomerRejection {
    fn into_response(self) -> Response {
        match self {
            Self::RedirectToLogin => Redirect::to("/auth/shopify/login").into_response(),
            Self::Unauthorized => StatusCode::UNAUTHORIZED.into_response(),
        }
    }
}

impl<S> FromRequestParts<S> for RequireShopifyCustomer
where
    S: Send + Sync,
{
    type Rejection = ShopifyCustomerRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Get the session from extensions (set by SessionManagerLayer)
        let session = parts
            .extensions
            .get::<Session>()
            .ok_or(ShopifyCustomerRejection::Unauthorized)?;

        // Get the customer token from the session
        let token: CustomerAccessToken = session
            .get(session_keys::SHOPIFY_CUSTOMER_TOKEN)
            .await
            .ok()
            .flatten()
            .ok_or_else(|| {
                // Check if this is an API request
                let is_api = parts.uri.path().starts_with("/api/");
                if is_api {
                    ShopifyCustomerRejection::Unauthorized
                } else {
                    ShopifyCustomerRejection::RedirectToLogin
                }
            })?;

        // TODO: Check if token is expired and attempt refresh
        // For now, we just return the token as-is

        Ok(Self(token))
    }
}

/// Extractor that optionally gets the Shopify customer token.
///
/// Unlike `RequireShopifyCustomer`, this does not reject the request if
/// the customer is not logged in via Shopify.
///
/// # Example
///
/// ```rust,ignore
/// async fn handler(
///     OptionalShopifyCustomer(token): OptionalShopifyCustomer,
/// ) -> impl IntoResponse {
///     match token {
///         Some(t) => format!("Customer authenticated!"),
///         None => "Guest visitor".to_string(),
///     }
/// }
/// ```
pub struct OptionalShopifyCustomer(pub Option<CustomerAccessToken>);

impl<S> FromRequestParts<S> for OptionalShopifyCustomer
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = match parts.extensions.get::<Session>() {
            Some(session) => session
                .get::<CustomerAccessToken>(session_keys::SHOPIFY_CUSTOMER_TOKEN)
                .await
                .ok()
                .flatten(),
            None => None,
        };

        Ok(Self(token))
    }
}

/// Helper to set the Shopify customer token in the session.
///
/// # Errors
///
/// Returns an error if the session cannot be modified.
pub async fn set_shopify_customer_token(
    session: &Session,
    token: &CustomerAccessToken,
) -> Result<(), tower_sessions::session::Error> {
    session
        .insert(session_keys::SHOPIFY_CUSTOMER_TOKEN, token)
        .await
}

/// Helper to clear the Shopify customer token from the session.
///
/// # Errors
///
/// Returns an error if the session cannot be modified.
pub async fn clear_shopify_customer_token(
    session: &Session,
) -> Result<(), tower_sessions::session::Error> {
    session
        .remove::<CustomerAccessToken>(session_keys::SHOPIFY_CUSTOMER_TOKEN)
        .await?;
    Ok(())
}

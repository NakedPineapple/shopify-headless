//! Authentication middleware and extractors.
//!
//! Provides extractors for requiring Shopify customer authentication in route handlers.

use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;

use crate::models::{CurrentCustomer, session_keys};

/// Extractor that requires Shopify customer authentication.
///
/// If the customer is not logged in, returns a redirect to the login page.
///
/// # Example
///
/// ```rust,ignore
/// async fn protected_handler(
///     RequireAuth(customer): RequireAuth,
/// ) -> impl IntoResponse {
///     format!("Hello, {}!", customer.email)
/// }
/// ```
pub struct RequireAuth(pub CurrentCustomer);

/// Error returned when authentication is required but the customer is not logged in.
pub enum AuthRejection {
    /// Redirect to login page (for HTML requests).
    RedirectToLogin,
    /// Unauthorized response (for API requests).
    Unauthorized,
}

impl IntoResponse for AuthRejection {
    fn into_response(self) -> Response {
        match self {
            Self::RedirectToLogin => Redirect::to("/auth/login").into_response(),
            Self::Unauthorized => StatusCode::UNAUTHORIZED.into_response(),
        }
    }
}

impl<S> FromRequestParts<S> for RequireAuth
where
    S: Send + Sync,
{
    type Rejection = AuthRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Get the session from extensions (set by SessionManagerLayer)
        let session = parts
            .extensions
            .get::<Session>()
            .ok_or(AuthRejection::Unauthorized)?;

        // Get the current customer from the session
        let customer: CurrentCustomer = session
            .get(session_keys::CURRENT_CUSTOMER)
            .await
            .ok()
            .flatten()
            .ok_or_else(|| {
                // Check if this is an API request
                let is_api = parts.uri.path().starts_with("/api/");
                if is_api {
                    AuthRejection::Unauthorized
                } else {
                    AuthRejection::RedirectToLogin
                }
            })?;

        // TODO: Check if token is expired and attempt refresh

        Ok(Self(customer))
    }
}

/// Extractor that optionally gets the current customer.
///
/// Unlike `RequireAuth`, this does not reject the request if the customer is not logged in.
///
/// # Example
///
/// ```rust,ignore
/// async fn handler(
///     OptionalAuth(customer): OptionalAuth,
/// ) -> impl IntoResponse {
///     match customer {
///         Some(c) => format!("Hello, {}!", c.email),
///         None => "Hello, guest!".to_string(),
///     }
/// }
/// ```
pub struct OptionalAuth(pub Option<CurrentCustomer>);

impl<S> FromRequestParts<S> for OptionalAuth
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let customer = match parts.extensions.get::<Session>() {
            Some(session) => session
                .get::<CurrentCustomer>(session_keys::CURRENT_CUSTOMER)
                .await
                .ok()
                .flatten(),
            None => None,
        };

        Ok(Self(customer))
    }
}

/// Helper to set the current customer in the session.
///
/// # Errors
///
/// Returns an error if the session cannot be modified.
pub async fn set_current_customer(
    session: &Session,
    customer: &CurrentCustomer,
) -> Result<(), tower_sessions::session::Error> {
    session
        .insert(session_keys::CURRENT_CUSTOMER, customer)
        .await
}

/// Helper to clear the current customer from the session (logout).
///
/// # Errors
///
/// Returns an error if the session cannot be modified.
pub async fn clear_current_customer(
    session: &Session,
) -> Result<(), tower_sessions::session::Error> {
    session
        .remove::<CurrentCustomer>(session_keys::CURRENT_CUSTOMER)
        .await?;
    Ok(())
}

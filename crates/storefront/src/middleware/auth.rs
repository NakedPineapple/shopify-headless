//! Authentication middleware and extractors.
//!
//! Provides extractors for requiring Shopify customer authentication in route handlers.

use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;
use tracing::{debug, instrument, warn};

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
        let path = parts.uri.path();
        debug!(path = %path, "RequireAuth: checking authentication");

        // Get the session from extensions (set by SessionManagerLayer)
        let session = parts.extensions.get::<Session>().ok_or_else(|| {
            warn!(path = %path, "RequireAuth: no session found in request extensions");
            AuthRejection::Unauthorized
        })?;

        // Get the current customer from the session
        let customer: CurrentCustomer = session
            .get(session_keys::CURRENT_CUSTOMER)
            .await
            .ok()
            .flatten()
            .ok_or_else(|| {
                // Check if this is an API request
                let is_api = path.starts_with("/api/");
                if is_api {
                    warn!(path = %path, "Access denied: no valid session for API request");
                    AuthRejection::Unauthorized
                } else {
                    debug!(path = %path, "Access denied: no valid session, redirecting to login");
                    AuthRejection::RedirectToLogin
                }
            })?;

        debug!(
            email = %customer.email,
            "RequireAuth: customer authenticated"
        );

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
        let path = parts.uri.path();
        debug!(path = %path, "OptionalAuth: checking for existing session");

        let customer = if let Some(session) = parts.extensions.get::<Session>() {
            session
                .get::<CurrentCustomer>(session_keys::CURRENT_CUSTOMER)
                .await
                .ok()
                .flatten()
        } else {
            debug!(path = %path, "OptionalAuth: no session in request extensions");
            None
        };

        if let Some(c) = &customer {
            debug!(email = %c.email, "OptionalAuth: customer found in session");
        } else {
            debug!(path = %path, "OptionalAuth: no customer in session");
        }

        Ok(Self(customer))
    }
}

/// Helper to set the current customer in the session.
///
/// # Errors
///
/// Returns an error if the session cannot be modified.
#[instrument(skip(session, customer), fields(email = %customer.email))]
pub async fn set_current_customer(
    session: &Session,
    customer: &CurrentCustomer,
) -> Result<(), tower_sessions::session::Error> {
    debug!("Setting current customer in session");
    let result = session
        .insert(session_keys::CURRENT_CUSTOMER, customer)
        .await;

    match &result {
        Ok(()) => debug!("Session created successfully"),
        Err(e) => warn!(error = %e, "Failed to create session"),
    }

    result
}

/// Helper to clear the current customer from the session (logout).
///
/// # Errors
///
/// Returns an error if the session cannot be modified.
#[instrument(skip(session))]
pub async fn clear_current_customer(
    session: &Session,
) -> Result<(), tower_sessions::session::Error> {
    debug!("Clearing current customer from session");
    let result = session
        .remove::<CurrentCustomer>(session_keys::CURRENT_CUSTOMER)
        .await;

    match &result {
        Ok(_) => debug!("Session destroyed successfully"),
        Err(e) => warn!(error = %e, "Failed to destroy session"),
    }

    result?;
    Ok(())
}

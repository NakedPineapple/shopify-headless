//! Authentication middleware and extractors.
//!
//! Provides extractors for requiring authentication in route handlers.

use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;

use crate::models::{CurrentUser, session_keys};

/// Extractor that requires authentication.
///
/// If the user is not logged in, returns a redirect to the login page.
///
/// # Example
///
/// ```rust,ignore
/// async fn protected_handler(
///     RequireAuth(user): RequireAuth,
/// ) -> impl IntoResponse {
///     format!("Hello, {}!", user.email)
/// }
/// ```
pub struct RequireAuth(pub CurrentUser);

/// Error returned when authentication is required but the user is not logged in.
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

        // Get the current user from the session
        let user: CurrentUser = session
            .get(session_keys::CURRENT_USER)
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

        Ok(Self(user))
    }
}

/// Extractor that optionally gets the current user.
///
/// Unlike `RequireAuth`, this does not reject the request if the user is not logged in.
///
/// # Example
///
/// ```rust,ignore
/// async fn handler(
///     OptionalAuth(user): OptionalAuth,
/// ) -> impl IntoResponse {
///     match user {
///         Some(u) => format!("Hello, {}!", u.email),
///         None => "Hello, guest!".to_string(),
///     }
/// }
/// ```
pub struct OptionalAuth(pub Option<CurrentUser>);

impl<S> FromRequestParts<S> for OptionalAuth
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let user = match parts.extensions.get::<Session>() {
            Some(session) => session
                .get::<CurrentUser>(session_keys::CURRENT_USER)
                .await
                .ok()
                .flatten(),
            None => None,
        };

        Ok(Self(user))
    }
}

/// Helper to set the current user in the session.
///
/// # Errors
///
/// Returns an error if the session cannot be modified.
pub async fn set_current_user(
    session: &Session,
    user: &CurrentUser,
) -> Result<(), tower_sessions::session::Error> {
    session.insert(session_keys::CURRENT_USER, user).await
}

/// Helper to clear the current user from the session (logout).
///
/// # Errors
///
/// Returns an error if the session cannot be modified.
pub async fn clear_current_user(session: &Session) -> Result<(), tower_sessions::session::Error> {
    session
        .remove::<CurrentUser>(session_keys::CURRENT_USER)
        .await?;
    Ok(())
}

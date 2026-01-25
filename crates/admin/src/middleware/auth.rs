//! Authentication middleware and extractors for admin.
//!
//! Provides extractors for requiring admin authentication in route handlers.

use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;

use crate::models::{CurrentAdmin, session_keys};

/// Extractor that requires admin authentication.
///
/// If the admin is not logged in, returns a redirect to the login page
/// for HTML requests, or 401 Unauthorized for API requests.
///
/// # Example
///
/// ```rust,ignore
/// async fn protected_handler(
///     RequireAdminAuth(admin): RequireAdminAuth,
/// ) -> impl IntoResponse {
///     format!("Hello, {}!", admin.name)
/// }
/// ```
pub struct RequireAdminAuth(pub CurrentAdmin);

/// Error returned when admin authentication is required but the user is not logged in.
pub enum AdminAuthRejection {
    /// Redirect to login page (for HTML requests).
    RedirectToLogin,
    /// Unauthorized response (for API requests).
    Unauthorized,
}

impl IntoResponse for AdminAuthRejection {
    fn into_response(self) -> Response {
        match self {
            Self::RedirectToLogin => Redirect::to("/auth/login").into_response(),
            Self::Unauthorized => StatusCode::UNAUTHORIZED.into_response(),
        }
    }
}

impl<S> FromRequestParts<S> for RequireAdminAuth
where
    S: Send + Sync,
{
    type Rejection = AdminAuthRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Get the session from extensions (set by SessionManagerLayer)
        let session = parts
            .extensions
            .get::<Session>()
            .ok_or(AdminAuthRejection::Unauthorized)?;

        // Get the current admin from the session
        let admin: CurrentAdmin = session
            .get(session_keys::CURRENT_ADMIN)
            .await
            .ok()
            .flatten()
            .ok_or_else(|| {
                // Check if this is an API request
                let is_api = parts.uri.path().starts_with("/api/");
                if is_api {
                    AdminAuthRejection::Unauthorized
                } else {
                    AdminAuthRejection::RedirectToLogin
                }
            })?;

        Ok(Self(admin))
    }
}

/// Extractor that optionally gets the current admin.
///
/// Unlike `RequireAdminAuth`, this does not reject the request if the admin is not logged in.
///
/// # Example
///
/// ```rust,ignore
/// async fn handler(
///     OptionalAdminAuth(admin): OptionalAdminAuth,
/// ) -> impl IntoResponse {
///     match admin {
///         Some(a) => format!("Hello, {}!", a.name),
///         None => "Hello, guest!".to_string(),
///     }
/// }
/// ```
pub struct OptionalAdminAuth(pub Option<CurrentAdmin>);

impl<S> FromRequestParts<S> for OptionalAdminAuth
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let admin = match parts.extensions.get::<Session>() {
            Some(session) => session
                .get::<CurrentAdmin>(session_keys::CURRENT_ADMIN)
                .await
                .ok()
                .flatten(),
            None => None,
        };

        Ok(Self(admin))
    }
}

/// Helper to set the current admin in the session.
///
/// # Errors
///
/// Returns an error if the session cannot be modified.
pub async fn set_current_admin(
    session: &Session,
    admin: &CurrentAdmin,
) -> Result<(), tower_sessions::session::Error> {
    session.insert(session_keys::CURRENT_ADMIN, admin).await
}

/// Helper to clear the current admin from the session (logout).
///
/// # Errors
///
/// Returns an error if the session cannot be modified.
pub async fn clear_current_admin(session: &Session) -> Result<(), tower_sessions::session::Error> {
    session
        .remove::<CurrentAdmin>(session_keys::CURRENT_ADMIN)
        .await?;
    Ok(())
}

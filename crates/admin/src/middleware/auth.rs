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

/// Extractor that requires super admin authentication.
///
/// If the admin is not logged in, redirects to login.
/// If the admin is not a super admin, returns 403 Forbidden.
///
/// # Example
///
/// ```rust,ignore
/// async fn super_admin_handler(
///     RequireSuperAdmin(admin): RequireSuperAdmin,
/// ) -> impl IntoResponse {
///     format!("Hello super admin {}!", admin.name)
/// }
/// ```
pub struct RequireSuperAdmin(pub CurrentAdmin);

/// Error returned when super admin authentication is required.
pub enum SuperAdminRejection {
    /// Redirect to login page (for HTML requests).
    RedirectToLogin,
    /// Unauthorized response (for API requests).
    Unauthorized,
    /// Forbidden - user is admin but not super admin.
    Forbidden,
}

impl IntoResponse for SuperAdminRejection {
    fn into_response(self) -> Response {
        match self {
            Self::RedirectToLogin => Redirect::to("/auth/login").into_response(),
            Self::Unauthorized => StatusCode::UNAUTHORIZED.into_response(),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "Only super admins can access this resource",
            )
                .into_response(),
        }
    }
}

impl<S> FromRequestParts<S> for RequireSuperAdmin
where
    S: Send + Sync,
{
    type Rejection = SuperAdminRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        use crate::models::AdminRole;

        // Get the session from extensions
        let session = parts
            .extensions
            .get::<Session>()
            .ok_or(SuperAdminRejection::Unauthorized)?;

        // Get the current admin from the session
        let admin: CurrentAdmin = session
            .get(session_keys::CURRENT_ADMIN)
            .await
            .ok()
            .flatten()
            .ok_or_else(|| {
                let is_api = parts.uri.path().starts_with("/api/");
                if is_api {
                    SuperAdminRejection::Unauthorized
                } else {
                    SuperAdminRejection::RedirectToLogin
                }
            })?;

        // Check for super admin role
        if admin.role != AdminRole::SuperAdmin {
            return Err(SuperAdminRejection::Forbidden);
        }

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

/// Check that the current user is a super admin.
///
/// Returns `Ok(())` if the user is authenticated and has the `SuperAdmin` role.
///
/// # Errors
///
/// Returns `Err(Response)` with a redirect to login if not authenticated,
/// or a 403 Forbidden response if the user is not a super admin.
pub async fn require_super_admin<S>(_state: &S, session: &Session) -> Result<(), Response>
where
    S: Send + Sync,
{
    use crate::models::AdminRole;

    let admin: CurrentAdmin = session
        .get(session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
        .ok_or_else(|| Redirect::to("/auth/login").into_response())?;

    if admin.role != AdminRole::SuperAdmin {
        return Err((
            StatusCode::FORBIDDEN,
            "Only super admins can access this resource",
        )
            .into_response());
    }

    Ok(())
}

//! Unified error handling with Sentry integration.
//!
//! Provides a unified `AppError` type that captures errors to Sentry before
//! responding to the client. All route handlers should return `Result<T, AppError>`.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

use crate::db::RepositoryError;
use crate::services::auth::AuthError;
use crate::shopify::ShopifyError;

/// Application-level error type for the storefront.
#[derive(Debug, Error)]
pub enum AppError {
    /// Database operation failed.
    #[error("Database error: {0}")]
    Database(#[from] RepositoryError),

    /// Shopify API operation failed.
    #[error("Shopify error: {0}")]
    Shopify(#[from] ShopifyError),

    /// Authentication operation failed.
    #[error("Auth error: {0}")]
    Auth(#[from] AuthError),

    /// Resource not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// User is not authenticated.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Bad request from client.
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Rate limited.
    #[error("Rate limited")]
    RateLimited,

    /// Internal server error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Capture server errors to Sentry
        if matches!(
            self,
            Self::Database(_) | Self::Internal(_) | Self::Shopify(_)
        ) {
            let event_id = sentry::capture_error(&self);
            tracing::error!(
                error = %self,
                sentry_event_id = %event_id,
                "Request error"
            );
        }

        let status = match &self {
            Self::Database(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Shopify(_) => StatusCode::BAD_GATEWAY,
            Self::Auth(err) => match err {
                AuthError::InvalidCredentials | AuthError::UserNotFound => StatusCode::UNAUTHORIZED,
                AuthError::UserAlreadyExists => StatusCode::CONFLICT,
                AuthError::WeakPassword(_) | AuthError::InvalidEmail(_) => StatusCode::BAD_REQUEST,
                AuthError::InvalidSessionState | AuthError::NoCredentials => {
                    StatusCode::UNAUTHORIZED
                }
                AuthError::CredentialNotFound => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        };

        // Don't expose internal error details to clients
        let message = match &self {
            Self::Database(_) | Self::Internal(_) => "Internal server error".to_string(),
            Self::Shopify(_) => "External service error".to_string(),
            Self::Auth(err) => match err {
                AuthError::InvalidCredentials | AuthError::UserNotFound => {
                    "Invalid credentials".to_string()
                }
                AuthError::UserAlreadyExists => {
                    "An account with this email already exists".to_string()
                }
                AuthError::WeakPassword(msg) => msg.clone(),
                AuthError::InvalidEmail(_) => "Invalid email address".to_string(),
                AuthError::NoCredentials => "No passkeys registered for this account".to_string(),
                AuthError::CredentialNotFound => "Credential not found".to_string(),
                AuthError::InvalidSessionState => "Session expired, please try again".to_string(),
                _ => "Authentication error".to_string(),
            },
            _ => self.to_string(),
        };

        (status, message).into_response()
    }
}

/// Result type alias for `AppError`.
pub type Result<T> = std::result::Result<T, AppError>;

/// Set the Sentry user context from a user ID.
///
/// Call this after successful authentication to associate errors with users.
pub fn set_sentry_user(user_id: &impl ToString, email: Option<&str>) {
    sentry::configure_scope(|scope| {
        scope.set_user(Some(sentry::User {
            id: Some(user_id.to_string()),
            email: email.map(String::from),
            ..Default::default()
        }));
    });
}

/// Clear the Sentry user context.
///
/// Call this on logout to stop associating errors with the user.
pub fn clear_sentry_user() {
    sentry::configure_scope(|scope| {
        scope.set_user(None);
    });
}

/// Add a breadcrumb for user actions.
///
/// Breadcrumbs appear in Sentry error reports to show the trail of user actions
/// leading up to an error.
///
/// # Example
///
/// ```rust,ignore
/// add_breadcrumb("navigation", "Viewed product page", Some(&[("product_id", "123")]));
/// ```
pub fn add_breadcrumb(category: &str, message: &str, data: Option<&[(&str, &str)]>) {
    let mut breadcrumb = sentry::Breadcrumb {
        category: Some(category.to_string()),
        message: Some(message.to_string()),
        level: sentry::Level::Info,
        ..Default::default()
    };

    if let Some(pairs) = data {
        for (key, value) in pairs {
            breadcrumb.data.insert(
                (*key).to_string(),
                serde_json::Value::String((*value).to_string()),
            );
        }
    }

    sentry::add_breadcrumb(breadcrumb);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_display() {
        let err = AppError::NotFound("product-123".to_string());
        assert_eq!(err.to_string(), "Not found: product-123");

        let err = AppError::BadRequest("invalid input".to_string());
        assert_eq!(err.to_string(), "Bad request: invalid input");
    }

    #[test]
    fn test_app_error_status_codes() {
        fn get_status(err: AppError) -> StatusCode {
            let response = err.into_response();
            response.status()
        }

        assert_eq!(
            get_status(AppError::NotFound("test".to_string())),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get_status(AppError::Unauthorized("test".to_string())),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            get_status(AppError::BadRequest("test".to_string())),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            get_status(AppError::RateLimited),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            get_status(AppError::Internal("test".to_string())),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}

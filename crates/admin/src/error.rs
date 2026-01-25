//! Unified error handling for admin.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

use crate::claude::ClaudeError;
use crate::db::RepositoryError;
use crate::shopify::AdminShopifyError;

/// Application-level error type for the admin panel.
#[derive(Debug, Error)]
pub enum AppError {
    /// Database operation failed.
    #[error("Database error: {0}")]
    Database(#[from] RepositoryError),

    /// Shopify API operation failed.
    #[error("Shopify error: {0}")]
    Shopify(#[from] AdminShopifyError),

    /// Claude API operation failed.
    #[error("Claude error: {0}")]
    Claude(#[from] ClaudeError),

    /// Resource not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// User is not authenticated.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// User lacks permission.
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// Bad request from client.
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Internal server error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Log server errors with Sentry
        if matches!(
            self,
            Self::Database(_) | Self::Internal(_) | Self::Shopify(_)
        ) {
            let event_id = sentry::capture_error(&self);
            tracing::error!(
                error = %self,
                sentry_event_id = %event_id,
                "Admin request error"
            );
        }

        let status = match &self {
            Self::Database(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Shopify(_) | Self::Claude(_) => StatusCode::BAD_GATEWAY,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
        };

        // Don't expose internal error details to clients
        let message = match &self {
            Self::Database(_) | Self::Internal(_) => "Internal server error".to_string(),
            Self::Shopify(_) => "External service error".to_string(),
            _ => self.to_string(),
        };

        (status, message).into_response()
    }
}

/// Set the Sentry user context from an admin user ID.
pub fn set_sentry_user(admin_user_id: i32, email: Option<&str>) {
    sentry::configure_scope(|scope| {
        scope.set_user(Some(sentry::User {
            id: Some(admin_user_id.to_string()),
            email: email.map(String::from),
            ..Default::default()
        }));
    });
}

/// Clear the Sentry user context.
pub fn clear_sentry_user() {
    sentry::configure_scope(|scope| {
        scope.set_user(None);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_display() {
        let err = AppError::NotFound("order-123".to_string());
        assert_eq!(err.to_string(), "Not found: order-123");

        let err = AppError::BadRequest("invalid input".to_string());
        assert_eq!(err.to_string(), "Bad request: invalid input");
    }

    #[test]
    fn test_app_error_status_codes() {
        // Test that errors map to correct HTTP status codes
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
            get_status(AppError::Forbidden("test".to_string())),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            get_status(AppError::BadRequest("test".to_string())),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            get_status(AppError::Internal("test".to_string())),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}

//! Unified error handling for admin.
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use axum::{
//!     http::StatusCode,
//!     response::{IntoResponse, Response},
//! };
//!
//! #[derive(Debug, thiserror::Error)]
//! pub enum AppError {
//!     #[error("Database error: {0}")]
//!     Database(#[from] sqlx::Error),
//!
//!     #[error("Shopify error: {0}")]
//!     Shopify(#[from] ShopifyError),
//!
//!     #[error("Claude error: {0}")]
//!     Claude(#[from] ClaudeError),
//!
//!     #[error("Not found: {0}")]
//!     NotFound(String),
//!
//!     #[error("Unauthorized: {0}")]
//!     Unauthorized(String),
//!
//!     #[error("Forbidden: {0}")]
//!     Forbidden(String),
//!
//!     #[error("Internal error: {0}")]
//!     Internal(String),
//! }
//!
//! impl IntoResponse for AppError {
//!     fn into_response(self) -> Response {
//!         // Capture to Sentry for server errors
//!         if matches!(self, Self::Database(_) | Self::Internal(_)) {
//!             let event_id = sentry::capture_error(&self);
//!             tracing::error!(
//!                 error = %self,
//!                 sentry_event_id = %event_id,
//!                 "Admin request error"
//!             );
//!         }
//!
//!         let status = match &self {
//!             Self::Database(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
//!             Self::Shopify(_) | Self::Claude(_) => StatusCode::BAD_GATEWAY,
//!             Self::NotFound(_) => StatusCode::NOT_FOUND,
//!             Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
//!             Self::Forbidden(_) => StatusCode::FORBIDDEN,
//!         };
//!
//!         (status, self.to_string()).into_response()
//!     }
//! }
//! ```

// TODO: Implement AppError enum

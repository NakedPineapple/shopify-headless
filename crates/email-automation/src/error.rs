//! Crate-level error types for the email automation service.

use thiserror::Error;

use crate::db::RepositoryError;
use crate::microsoft_graph::M365Error;

/// Top-level error type for the email automation service.
#[derive(Debug, Error)]
pub enum AppError {
    /// Database operation failed.
    #[error("database error: {0}")]
    Database(#[from] RepositoryError),

    /// Microsoft Graph API operation failed.
    #[error("Microsoft Graph error: {0}")]
    MicrosoftGraph(#[from] M365Error),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

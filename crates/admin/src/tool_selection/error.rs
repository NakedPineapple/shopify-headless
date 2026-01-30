//! Error types for tool selection.

use thiserror::Error;

use crate::db::RepositoryError;

/// Errors that can occur during tool selection.
#[derive(Debug, Error)]
pub enum ToolSelectionError {
    /// Failed to classify domains.
    #[error("domain classification failed: {0}")]
    Classification(String),

    /// Failed to generate embeddings.
    #[error("embedding generation failed: {0}")]
    Embedding(String),

    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Repository error.
    #[error("repository error: {0}")]
    Repository(#[from] RepositoryError),

    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON parsing failed.
    #[error("JSON parsing failed: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid response from API.
    #[error("invalid API response: {0}")]
    InvalidResponse(String),

    /// No tools found for query.
    #[error("no tools found for the given query")]
    NoToolsFound,

    /// IO error (file read/write).
    #[error("IO error: {0}")]
    Io(String),

    /// Configuration error (YAML parsing, validation).
    #[error("configuration error: {0}")]
    Config(String),
}

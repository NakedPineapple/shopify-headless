//! Error types for seed operations.

use thiserror::Error;

/// Errors that can occur during seeding operations.
#[derive(Debug, Error)]
pub enum SeedError {
    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Embedding generation error.
    #[error("embedding error: {0}")]
    Embedding(#[from] naked_pineapple_services::openai::EmbeddingError),

    /// IO error.
    #[error("IO error: {0}")]
    Io(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),
}

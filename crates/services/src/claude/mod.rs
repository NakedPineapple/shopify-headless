//! Claude API client for chat interactions.
//!
//! Provides both streaming and non-streaming access to the Anthropic Messages API.

mod client;
mod error;
pub mod types;

pub use client::ClaudeClient;
pub use error::{ApiError, ApiErrorResponse, ClaudeError};
pub use types::*;

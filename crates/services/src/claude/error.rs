//! Error types for the Claude API client.

use thiserror::Error;

/// Errors that can occur when interacting with the Claude API.
#[derive(Debug, Error)]
pub enum ClaudeError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Claude API returned an error.
    #[error("API error ({error_type}): {message}")]
    Api {
        /// Error type from the API.
        error_type: String,
        /// Error message.
        message: String,
    },

    /// Rate limited by the API.
    #[error("rate limited, retry after {0} seconds")]
    RateLimited(u64),

    /// Authentication failed.
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// Failed to parse response.
    #[error("parse error: {0}")]
    Parse(String),

    /// Stream error.
    #[error("stream error: {0}")]
    Stream(String),

    /// Tool execution failed.
    #[error("tool execution error: {0}")]
    ToolExecution(String),
}

/// API error response from Claude.
#[derive(Debug, serde::Deserialize)]
pub struct ApiErrorResponse {
    /// Error type.
    #[serde(rename = "type")]
    pub error_type: String,
    /// Nested error details.
    pub error: ApiError,
}

/// Nested error details.
#[derive(Debug, serde::Deserialize)]
pub struct ApiError {
    /// Error type.
    #[serde(rename = "type")]
    pub error_type: String,
    /// Error message.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_error_display() {
        let err = ClaudeError::RateLimited(60);
        assert_eq!(err.to_string(), "rate limited, retry after 60 seconds");

        let err = ClaudeError::Api {
            error_type: "invalid_request_error".to_string(),
            message: "Invalid API key".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "API error (invalid_request_error): Invalid API key"
        );
    }

    #[test]
    fn test_api_error_deserialization() {
        let json = r#"{
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": "max_tokens is too large"
            }
        }"#;

        let response: ApiErrorResponse = serde_json::from_str(json).expect("deserialize");
        assert_eq!(response.error_type, "error");
        assert_eq!(response.error.error_type, "invalid_request_error");
        assert_eq!(response.error.message, "max_tokens is too large");
    }
}

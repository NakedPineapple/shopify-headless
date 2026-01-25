//! Claude API client for chat interactions.
//!
//! Provides both streaming and non-streaming API access for the Anthropic Messages API.

use std::sync::Arc;

use async_stream::stream;
use futures::Stream;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use secrecy::ExposeSecret;
use tracing::instrument;

use crate::config::ClaudeConfig;

use super::error::{ApiErrorResponse, ClaudeError};
use super::types::{ChatRequest, ChatResponse, Message, StreamEvent, Tool};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Claude API client.
///
/// Provides methods to interact with the Anthropic Messages API for chat
/// completions with optional tool use.
#[derive(Clone)]
pub struct ClaudeClient {
    inner: Arc<ClaudeClientInner>,
}

struct ClaudeClientInner {
    client: reqwest::Client,
    model: String,
}

impl ClaudeClient {
    /// Create a new Claude client.
    ///
    /// # Arguments
    ///
    /// * `config` - Claude API configuration containing API key and model
    ///
    /// # Panics
    ///
    /// Panics if the API key contains invalid header characters.
    #[must_use]
    pub fn new(config: &ClaudeConfig) -> Self {
        let api_key = config.api_key.expose_secret();

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(api_key).expect("Invalid API key for header"),
        );
        headers.insert(
            "anthropic-version",
            HeaderValue::from_static(ANTHROPIC_VERSION),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            inner: Arc::new(ClaudeClientInner {
                client,
                model: config.model.clone(),
            }),
        }
    }

    /// Send a chat request and get a complete response.
    ///
    /// This is the non-streaming API, suitable for tool use loops where
    /// you need to process the complete response before continuing.
    ///
    /// # Arguments
    ///
    /// * `messages` - Conversation history
    /// * `system` - Optional system prompt
    /// * `tools` - Optional list of available tools
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an error response.
    #[instrument(skip(self, messages, tools), fields(model = %self.inner.model))]
    pub async fn chat(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Option<Vec<Tool>>,
    ) -> Result<ChatResponse, ClaudeError> {
        let request = ChatRequest {
            model: self.inner.model.clone(),
            max_tokens: DEFAULT_MAX_TOKENS,
            messages,
            system,
            tools,
            stream: None,
        };

        let response = self
            .inner
            .client
            .post(ANTHROPIC_API_URL)
            .json(&request)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Send a chat request and get a streaming response.
    ///
    /// Returns a stream of events for real-time display of the response.
    ///
    /// # Arguments
    ///
    /// * `messages` - Conversation history
    /// * `system` - Optional system prompt
    /// * `tools` - Optional list of available tools
    ///
    /// # Errors
    ///
    /// Returns an error if the initial request fails.
    #[instrument(skip(self, messages, tools), fields(model = %self.inner.model))]
    pub async fn chat_stream(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        tools: Option<Vec<Tool>>,
    ) -> Result<impl Stream<Item = Result<StreamEvent, ClaudeError>>, ClaudeError> {
        let request = ChatRequest {
            model: self.inner.model.clone(),
            max_tokens: DEFAULT_MAX_TOKENS,
            messages,
            system,
            tools,
            stream: Some(true),
        };

        let response = self
            .inner
            .client
            .post(ANTHROPIC_API_URL)
            .json(&request)
            .send()
            .await?;

        // Check for error responses before streaming
        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_status(status, response).await);
        }

        // Return a stream that parses SSE events
        Ok(stream! {
            use futures::StreamExt;

            let mut buffer = String::new();
            let mut byte_stream = std::pin::pin!(response.bytes_stream());

            while let Some(chunk_result) = byte_stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        let text = match std::str::from_utf8(&chunk) {
                            Ok(t) => t,
                            Err(e) => {
                                yield Err(ClaudeError::Parse(format!("Invalid UTF-8: {e}")));
                                continue;
                            }
                        };

                        buffer.push_str(text);

                        // Process complete SSE events
                        while let Some(event) = extract_sse_event(&mut buffer) {
                            if let Some(parsed) = parse_sse_event(&event) {
                                match parsed {
                                    Ok(stream_event) => yield Ok(stream_event),
                                    Err(e) => yield Err(e),
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(ClaudeError::Stream(e.to_string()));
                    }
                }
            }
        })
    }

    /// Handle a successful response.
    async fn handle_response(
        &self,
        response: reqwest::Response,
    ) -> Result<ChatResponse, ClaudeError> {
        let status = response.status();

        if status.is_success() {
            let body = response.text().await?;
            serde_json::from_str(&body)
                .map_err(|e| ClaudeError::Parse(format!("Failed to parse response: {e}")))
        } else {
            Err(self.handle_error_status(status, response).await)
        }
    }

    /// Handle an error status code.
    async fn handle_error_status(
        &self,
        status: reqwest::StatusCode,
        response: reqwest::Response,
    ) -> ClaudeError {
        // Check for rate limiting
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(60);
            return ClaudeError::RateLimited(retry_after);
        }

        // Check for unauthorized
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return ClaudeError::Unauthorized("Invalid API key".to_string());
        }

        // Try to parse API error response
        match response.text().await {
            Ok(body) => {
                if let Ok(api_error) = serde_json::from_str::<ApiErrorResponse>(&body) {
                    ClaudeError::Api {
                        error_type: api_error.error.error_type,
                        message: api_error.error.message,
                    }
                } else {
                    ClaudeError::Api {
                        error_type: "unknown".to_string(),
                        message: body,
                    }
                }
            }
            Err(e) => ClaudeError::Http(e),
        }
    }
}

/// Extract a complete SSE event from the buffer.
///
/// Returns `Some(event)` if a complete event was found (and removes it from buffer),
/// or `None` if no complete event is available yet.
fn extract_sse_event(buffer: &mut String) -> Option<String> {
    // SSE events are separated by double newlines
    buffer.find("\n\n").map(|idx| {
        let event = buffer[..idx].to_string();
        *buffer = buffer[idx + 2..].to_string();
        event
    })
}

/// Parse an SSE event string into a `StreamEvent`.
fn parse_sse_event(event: &str) -> Option<Result<StreamEvent, ClaudeError>> {
    // Skip empty events
    if event.trim().is_empty() {
        return None;
    }

    // Parse SSE format: "event: <type>\ndata: <json>"
    let mut data_line = None;

    for line in event.lines() {
        if let Some(stripped) = line.strip_prefix("data: ") {
            data_line = Some(stripped);
        }
    }

    let data = data_line?;

    // Handle [DONE] marker (not used by Claude but handle it anyway)
    if data == "[DONE]" {
        return None;
    }

    // Parse the JSON data
    match serde_json::from_str::<StreamEvent>(data) {
        Ok(stream_event) => Some(Ok(stream_event)),
        Err(e) => Some(Err(ClaudeError::Parse(format!(
            "Failed to parse stream event: {e}"
        )))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sse_event() {
        let mut buffer = "event: message_start\ndata: {}\n\nevent: ping\ndata: {}\n\n".to_string();

        let event1 = extract_sse_event(&mut buffer);
        assert!(event1.is_some());
        assert!(event1.expect("no event").contains("message_start"));

        let event2 = extract_sse_event(&mut buffer);
        assert!(event2.is_some());

        let event3 = extract_sse_event(&mut buffer);
        assert!(event3.is_none());
    }

    #[test]
    fn test_extract_sse_event_incomplete() {
        let mut buffer = "event: message_start\ndata: {\"partial".to_string();
        let event = extract_sse_event(&mut buffer);
        assert!(event.is_none());
        assert_eq!(buffer, "event: message_start\ndata: {\"partial");
    }

    #[test]
    fn test_parse_sse_event_ping() {
        let event = "event: ping\ndata: {\"type\":\"ping\"}";
        let result = parse_sse_event(event);
        assert!(result.is_some());
        let stream_event = result.expect("no result").expect("parse error");
        assert!(matches!(stream_event, StreamEvent::Ping));
    }

    #[test]
    fn test_parse_sse_event_empty() {
        let event = "";
        let result = parse_sse_event(event);
        assert!(result.is_none());
    }

    #[test]
    fn test_claude_client_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<ClaudeClient>();
    }

    #[test]
    fn test_claude_client_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ClaudeClient>();
    }
}

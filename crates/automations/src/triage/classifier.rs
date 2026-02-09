//! AI email classifier using Claude.
//!
//! Makes a single non-streaming Claude API call with the `classify_email` tool
//! to classify inbound emails and extract key entities.

use naked_pineapple_services::claude::{
    ChatResponse, ClaudeClient, ClaudeError, ContentBlock, Message, MessageContent, StopReason,
};
use tracing::{debug, instrument, warn};

use super::tools::{classification_system_prompt, classify_email_tool};
use super::types::{ClassificationResult, EmailClassification, ExtractedEntities};

/// Context provided to the classifier for a single email.
pub struct EmailContext {
    /// Sender email address.
    pub from_address: String,
    /// Sender display name (if available).
    pub from_name: Option<String>,
    /// Email subject line.
    pub subject: String,
    /// Email body text (truncated to 2000 chars).
    pub body: String,
    /// Prior messages in this conversation thread (for context).
    pub thread_context: Vec<ThreadMessage>,
}

/// A prior message in the conversation thread.
pub struct ThreadMessage {
    /// Sender address.
    pub from: String,
    /// Body preview.
    pub body_preview: String,
}

/// Classify an inbound email using Claude AI.
///
/// Makes a single non-streaming API call with the `classify_email` tool.
/// Claude is instructed to call the tool exactly once with its classification.
///
/// # Errors
///
/// Returns `ClaudeError` if the API call fails or the response cannot be parsed.
#[instrument(skip(claude, context), fields(from = %context.from_address, subject = %context.subject))]
pub async fn classify_email(
    claude: &ClaudeClient,
    context: &EmailContext,
) -> Result<ClassificationResult, ClaudeError> {
    let user_message = build_classification_prompt(context);

    let messages = vec![Message {
        role: "user".to_string(),
        content: MessageContent::Text(user_message),
    }];

    let tools = vec![classify_email_tool()];
    let system = Some(classification_system_prompt());

    debug!("sending classification request to Claude");
    let response = claude.chat(messages, system, Some(tools)).await?;

    parse_classification_response(&response)
}

/// Build the user prompt for classification.
fn build_classification_prompt(context: &EmailContext) -> String {
    use std::fmt::Write;

    let mut prompt = String::with_capacity(3000);

    prompt.push_str("Classify the following inbound email:\n\n");
    let _ = write!(prompt, "From: {} ", context.from_address);
    if let Some(name) = &context.from_name {
        let _ = write!(prompt, "({name})");
    }
    prompt.push('\n');
    let _ = writeln!(prompt, "Subject: {}\n", context.subject);

    // Truncate body to 2000 chars
    let body = if context.body.len() > 2000 {
        format!("{}...(truncated)", &context.body[..2000])
    } else {
        context.body.clone()
    };
    let _ = writeln!(prompt, "Body:\n{body}");

    // Add thread context if available
    if !context.thread_context.is_empty() {
        prompt.push_str("\n--- Previous messages in this thread ---\n");
        for msg in &context.thread_context {
            let preview = if msg.body_preview.len() > 500 {
                format!("{}...", &msg.body_preview[..500])
            } else {
                msg.body_preview.clone()
            };
            let _ = write!(prompt, "From: {}\n{preview}\n---\n", msg.from);
        }
    }

    prompt
}

/// Parse the classification response from Claude.
fn parse_classification_response(
    response: &ChatResponse,
) -> Result<ClassificationResult, ClaudeError> {
    // Look for the tool_use content block
    let tool_input = response
        .content
        .iter()
        .find_map(|block| match block {
            ContentBlock::ToolUse { name, input, .. } if name == "classify_email" => Some(input),
            _ => None,
        })
        .ok_or_else(|| ClaudeError::Parse("Claude did not call classify_email tool".to_string()))?;

    // Verify the response was a tool_use stop
    if response.stop_reason != Some(StopReason::ToolUse) {
        warn!(
            stop_reason = ?response.stop_reason,
            "unexpected stop reason for classification"
        );
    }

    parse_tool_input(tool_input)
}

/// Parse the tool input JSON into a `ClassificationResult`.
fn parse_tool_input(input: &serde_json::Value) -> Result<ClassificationResult, ClaudeError> {
    let classification_str = input["classification"]
        .as_str()
        .ok_or_else(|| ClaudeError::Parse("missing classification field".to_string()))?;

    let classification: EmailClassification =
        serde_json::from_value(serde_json::Value::String(classification_str.to_string()))
            .map_err(|e| ClaudeError::Parse(format!("invalid classification: {e}")))?;

    let confidence = input["confidence"]
        .as_f64()
        .ok_or_else(|| ClaudeError::Parse("missing confidence field".to_string()))?;

    let reasoning = input["reasoning"]
        .as_str()
        .unwrap_or("No reasoning provided")
        .to_string();

    let sub_category = input["sub_category"].as_str().map(String::from);

    let extracted_entities = ExtractedEntities {
        order_numbers: extract_string_array(&input["order_numbers"]),
        product_names: extract_string_array(&input["product_names"]),
        tracking_numbers: extract_string_array(&input["tracking_numbers"]),
        customer_name: input["customer_name"].as_str().map(String::from),
    };

    debug!(
        %classification,
        confidence,
        "email classified"
    );

    Ok(ClassificationResult {
        classification,
        sub_category,
        confidence,
        reasoning,
        extracted_entities,
    })
}

/// Extract a `Vec<String>` from a JSON array value.
fn extract_string_array(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_tool_input_valid() {
        let input = json!({
            "classification": "order_inquiry",
            "confidence": 0.92,
            "reasoning": "Customer is asking about order status",
            "order_numbers": ["#1234", "#5678"],
            "customer_name": "Jane Doe"
        });

        let result = parse_tool_input(&input).expect("should parse");
        assert_eq!(result.classification, EmailClassification::OrderInquiry);
        assert!((result.confidence - 0.92).abs() < f64::EPSILON);
        assert_eq!(result.extracted_entities.order_numbers.len(), 2);
        assert_eq!(
            result.extracted_entities.customer_name.as_deref(),
            Some("Jane Doe")
        );
    }

    #[test]
    fn test_parse_tool_input_minimal() {
        let input = json!({
            "classification": "spam",
            "confidence": 0.99,
            "reasoning": "Obvious spam"
        });

        let result = parse_tool_input(&input).expect("should parse");
        assert_eq!(result.classification, EmailClassification::Spam);
        assert!(result.extracted_entities.order_numbers.is_empty());
    }

    #[test]
    fn test_parse_tool_input_missing_classification() {
        let input = json!({
            "confidence": 0.9,
            "reasoning": "test"
        });

        let result = parse_tool_input(&input);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_classification_prompt_truncates_body() {
        let long_body = "x".repeat(3000);
        let context = EmailContext {
            from_address: "test@example.com".to_string(),
            from_name: Some("Test User".to_string()),
            subject: "Test Subject".to_string(),
            body: long_body,
            thread_context: vec![],
        };

        let prompt = build_classification_prompt(&context);
        assert!(prompt.contains("(truncated)"));
        assert!(prompt.len() < 3500);
    }
}

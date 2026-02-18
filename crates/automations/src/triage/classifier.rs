//! AI email classifier using Claude.
//!
//! Multi-turn classification: Claude can optionally call `lookup_contact` to
//! query the contact graph before returning a JSON classification. Supports
//! up to 3 iterations.

use askama::Template;
use naked_pineapple_services::claude::{
    ChatResponse, ClaudeClient, ClaudeError, ContentBlock, Message, MessageContent, StopReason,
};
use sqlx::PgPool;
use tracing::{debug, instrument, warn};

use super::extract_json;
use super::tools::{classification_system_prompt, lookup_contact_tool};
use super::truncate_with_ellipsis;
use super::types::{ClassificationResult, EmailClassification, ExtractedEntities};
use crate::db::contact_graph;

/// Maximum number of tool-use loop iterations before giving up.
const MAX_TOOL_ITERATIONS: usize = 3;

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
    /// Known sender information from the contact graph (if available).
    pub sender_context: Option<String>,
    /// Similar past emails from RAG search (if available).
    pub rag_context: Option<String>,
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
/// Multi-turn loop: provides `lookup_contact` tool for optional contact graph
/// queries. Claude returns the classification as JSON text when ready.
/// The loop runs up to 3 iterations.
///
/// # Errors
///
/// Returns `ClaudeError` if the API call fails or the response cannot be parsed.
#[instrument(skip(claude, pool, context), fields(from = %context.from_address, subject = %context.subject))]
pub async fn classify_email(
    claude: &ClaudeClient,
    pool: &PgPool,
    context: &EmailContext,
) -> Result<ClassificationResult, ClaudeError> {
    let user_message = build_classification_prompt(context);

    let mut messages = vec![Message {
        role: "user".to_string(),
        content: MessageContent::Text(user_message),
    }];

    let tools = vec![lookup_contact_tool()];
    let system = Some(classification_system_prompt());

    for iteration in 0..MAX_TOOL_ITERATIONS {
        debug!(iteration, "sending classification request to Claude");
        let response = claude
            .chat(messages.clone(), system.clone(), Some(tools.clone()))
            .await?;

        match response.stop_reason {
            Some(StopReason::ToolUse) => {
                // Must be lookup_contact — handle it and continue
                let (new_messages, handled) = handle_tool_calls(pool, &response, &messages).await?;
                if !handled {
                    return Err(ClaudeError::Parse(
                        "unexpected tool call (not lookup_contact)".to_string(),
                    ));
                }
                messages = new_messages;
            }
            Some(StopReason::EndTurn) => {
                // Claude is done — parse classification from text JSON
                let text = extract_text_content(&response)?;
                let json = extract_json(&text)?;
                return parse_classification(&json);
            }
            _ => {
                return Err(ClaudeError::Parse(format!(
                    "unexpected stop reason: {:?}",
                    response.stop_reason
                )));
            }
        }
    }

    Err(ClaudeError::Parse(
        "max tool iterations reached without classification".to_string(),
    ))
}

/// Extract concatenated text content from a Claude response.
fn extract_text_content(response: &ChatResponse) -> Result<String, ClaudeError> {
    let text: String = response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");

    if text.is_empty() {
        return Err(ClaudeError::Parse(
            "no text content in classification response".to_string(),
        ));
    }

    Ok(text)
}

/// Handle tool calls in the response, executing `lookup_contact` and building
/// the next message list. Returns `(new_messages, was_handled)`.
async fn handle_tool_calls(
    pool: &PgPool,
    response: &ChatResponse,
    previous_messages: &[Message],
) -> Result<(Vec<Message>, bool), ClaudeError> {
    let mut messages = previous_messages.to_vec();
    let mut tool_results = Vec::new();
    let mut handled = false;

    // Add assistant response
    messages.push(Message {
        role: "assistant".to_string(),
        content: MessageContent::Blocks(response.content.clone()),
    });

    for block in &response.content {
        if let ContentBlock::ToolUse { id, name, input } = block
            && name == "lookup_contact"
        {
            let result_text = execute_contact_lookup(pool, input).await;
            tool_results.push(ContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: result_text,
                is_error: None,
            });
            handled = true;
        }
    }

    if !tool_results.is_empty() {
        messages.push(Message {
            role: "user".to_string(),
            content: MessageContent::Blocks(tool_results),
        });
    }

    Ok((messages, handled))
}

/// Execute a contact graph lookup and return formatted results.
async fn execute_contact_lookup(pool: &PgPool, input: &serde_json::Value) -> String {
    let query = input["query"].as_str().unwrap_or("");
    if query.is_empty() {
        return "No query provided.".to_string();
    }

    let contacts = match contact_graph::search(pool, query).await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "contact graph search failed");
            return format!("Search error: {e}");
        }
    };

    if contacts.is_empty() {
        return format!("No contacts found matching '{query}'.");
    }

    let mut results = Vec::new();
    for contact in &contacts {
        let Ok(neighborhood) = contact_graph::get_neighborhood(pool, contact.id, 2).await else {
            continue;
        };
        results.push(contact_graph::format_graph_context(&neighborhood));
    }

    results.join("\n---\n")
}

/// Pre-truncated thread message for the classification prompt template.
struct ThreadPromptMsg {
    from: String,
    preview: String,
}

/// Askama template for the email classification user prompt.
#[derive(Template)]
#[template(path = "prompts/classify_email.txt")]
struct ClassifyEmailPrompt<'a> {
    from_address: &'a str,
    from_name: Option<&'a str>,
    subject: &'a str,
    body: String,
    thread_messages: Vec<ThreadPromptMsg>,
    sender_context: Option<&'a str>,
    rag_context: Option<&'a str>,
}

/// Build the user prompt for classification.
fn build_classification_prompt(context: &EmailContext) -> String {
    let thread_messages: Vec<ThreadPromptMsg> = context
        .thread_context
        .iter()
        .map(|m| ThreadPromptMsg {
            from: m.from.clone(),
            preview: truncate_with_ellipsis(&m.body_preview, 500),
        })
        .collect();

    let template = ClassifyEmailPrompt {
        from_address: &context.from_address,
        from_name: context.from_name.as_deref(),
        subject: &context.subject,
        body: truncate_with_ellipsis(&context.body, 2000),
        thread_messages,
        sender_context: context.sender_context.as_deref(),
        rag_context: context.rag_context.as_deref(),
    };

    template.render().expect("classification prompt template")
}

/// Parse a classification JSON value into a `ClassificationResult`.
fn parse_classification(input: &serde_json::Value) -> Result<ClassificationResult, ClaudeError> {
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
    fn test_parse_classification_valid() {
        let input = json!({
            "classification": "order_inquiry",
            "confidence": 0.92,
            "reasoning": "Customer is asking about order status",
            "order_numbers": ["#1234", "#5678"],
            "customer_name": "Jane Doe"
        });

        let result = parse_classification(&input).expect("should parse");
        assert_eq!(result.classification, EmailClassification::OrderInquiry);
        assert!((result.confidence - 0.92).abs() < f64::EPSILON);
        assert_eq!(result.extracted_entities.order_numbers.len(), 2);
        assert_eq!(
            result.extracted_entities.customer_name.as_deref(),
            Some("Jane Doe")
        );
    }

    #[test]
    fn test_parse_classification_minimal() {
        let input = json!({
            "classification": "spam",
            "confidence": 0.99,
            "reasoning": "Obvious spam"
        });

        let result = parse_classification(&input).expect("should parse");
        assert_eq!(result.classification, EmailClassification::Spam);
        assert!(result.extracted_entities.order_numbers.is_empty());
    }

    #[test]
    fn test_parse_classification_missing_classification() {
        let input = json!({
            "confidence": 0.9,
            "reasoning": "test"
        });

        let result = parse_classification(&input);
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
            sender_context: None,
            rag_context: None,
        };

        let prompt = build_classification_prompt(&context);
        assert!(prompt.contains("(truncated)"));
        assert!(prompt.len() < 3500);
    }
}

//! Draft response composer using Claude AI.
//!
//! After classification, if the email needs a response (order inquiries,
//! product questions, etc.), this module uses Claude to compose a draft
//! reply that is queued for Slack review before sending.

use naked_pineapple_services::claude::{
    ClaudeClient, ClaudeError, ContentBlock, Message, MessageContent, StopReason,
};
use tracing::{debug, instrument, warn};

use super::tools::{compose_reply_tool, response_system_prompt};
use super::types::ClassificationResult;

/// A draft response composed by Claude for Slack review.
#[derive(Debug, Clone)]
pub struct DraftResponse {
    /// Plain text reply body.
    pub body_text: String,
}

/// Context for composing a draft response.
pub struct ResponseContext {
    /// The original email's sender address.
    pub from_address: String,
    /// Sender display name.
    pub from_name: Option<String>,
    /// Original email subject.
    pub subject: String,
    /// Original email body.
    pub body: String,
    /// Classification result from the classifier.
    pub classification: ClassificationResult,
    /// Pre-formatted Shopify data (orders, products) for response context.
    pub shopify_context: Option<String>,
}

/// Compose a draft response to an inbound email using Claude AI.
///
/// # Errors
///
/// Returns `ClaudeError` if the API call fails or the response cannot be parsed.
#[instrument(
    skip(claude, context),
    fields(
        from = %context.from_address,
        classification = %context.classification.classification
    )
)]
pub async fn compose_draft_response(
    claude: &ClaudeClient,
    context: &ResponseContext,
) -> Result<DraftResponse, ClaudeError> {
    let user_message = build_response_prompt(context);

    let messages = vec![Message {
        role: "user".to_string(),
        content: MessageContent::Text(user_message),
    }];

    let tools = vec![compose_reply_tool()];
    let system = Some(response_system_prompt());

    debug!("sending response composition request to Claude");
    let response = claude.chat(messages, system, Some(tools)).await?;

    // Look for the compose_reply tool call
    let tool_input = response
        .content
        .iter()
        .find_map(|block| match block {
            ContentBlock::ToolUse { name, input, .. } if name == "compose_reply" => Some(input),
            _ => None,
        })
        .ok_or_else(|| ClaudeError::Parse("Claude did not call compose_reply tool".to_string()))?;

    if response.stop_reason != Some(StopReason::ToolUse) {
        warn!(
            stop_reason = ?response.stop_reason,
            "unexpected stop reason for response composition"
        );
    }

    let body_text = tool_input["body_text"]
        .as_str()
        .ok_or_else(|| ClaudeError::Parse("missing body_text in compose_reply".to_string()))?
        .to_string();

    debug!("draft response composed");

    Ok(DraftResponse { body_text })
}

/// Build the user prompt for response composition.
fn build_response_prompt(context: &ResponseContext) -> String {
    use std::fmt::Write;

    let mut prompt = String::with_capacity(4000);

    prompt.push_str("Compose a reply to the following customer email.\n\n");
    let _ = writeln!(
        prompt,
        "Classification: {} (confidence: {:.0}%)",
        context.classification.classification,
        context.classification.confidence * 100.0
    );

    if !context
        .classification
        .extracted_entities
        .order_numbers
        .is_empty()
    {
        let _ = writeln!(
            prompt,
            "Referenced orders: {}",
            context
                .classification
                .extracted_entities
                .order_numbers
                .join(", ")
        );
    }

    if !context
        .classification
        .extracted_entities
        .product_names
        .is_empty()
    {
        let _ = writeln!(
            prompt,
            "Referenced products: {}",
            context
                .classification
                .extracted_entities
                .product_names
                .join(", ")
        );
    }

    let _ = write!(prompt, "\nFrom: {} ", context.from_address);
    if let Some(name) = &context.from_name {
        let _ = write!(prompt, "({name})");
    }
    prompt.push('\n');
    let _ = writeln!(prompt, "Subject: {}\n", context.subject);

    let body = if context.body.len() > 2000 {
        format!("{}...(truncated)", &context.body[..2000])
    } else {
        context.body.clone()
    };
    let _ = writeln!(prompt, "Body:\n{body}");

    if let Some(shopify) = &context.shopify_context {
        let _ = write!(prompt, "\n--- Real-time Shopify Data ---\n{shopify}\n");
        prompt.push_str("Use the Shopify data above to provide accurate, specific answers. ");
        prompt.push_str("Reference real order statuses, tracking numbers, and product details.\n");
    }

    prompt
        .push_str("\nDraft a professional, helpful reply. Sign off as 'The Naked Pineapple Team'.");

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triage::types::{EmailClassification, ExtractedEntities};

    #[test]
    fn test_build_response_prompt_includes_context() {
        let context = ResponseContext {
            from_address: "jane@example.com".to_string(),
            from_name: Some("Jane".to_string()),
            subject: "Where is my order?".to_string(),
            body: "I ordered last week and haven't received it.".to_string(),
            classification: ClassificationResult {
                classification: EmailClassification::OrderInquiry,
                sub_category: None,
                confidence: 0.95,
                reasoning: "Asking about order status".to_string(),
                extracted_entities: ExtractedEntities {
                    order_numbers: vec!["#1234".to_string()],
                    ..ExtractedEntities::default()
                },
            },
            shopify_context: None,
        };

        let prompt = build_response_prompt(&context);
        assert!(prompt.contains("Order Inquiry"));
        assert!(prompt.contains("#1234"));
        assert!(prompt.contains("jane@example.com"));
        assert!(prompt.contains("(Jane)"));
    }
}

//! Draft response composer using Claude AI.
//!
//! After classification, if the email needs a response (order inquiries,
//! product questions, etc.), this module uses Claude to compose a draft
//! reply that is queued for Slack review before sending.

use askama::Template;
use naked_pineapple_services::claude::{
    ClaudeClient, ClaudeError, ContentBlock, Message, MessageContent,
};
use tracing::{debug, instrument};

use crate::triage::extract_json;
use crate::triage::tools::response_system_prompt;
use crate::triage::truncate_with_ellipsis;
use crate::triage::types::ClassificationResult;

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

    let system = Some(response_system_prompt());

    debug!("sending response composition request to Claude");
    let response = claude.chat(messages, system, None).await?;

    // Extract text content from response
    let text: String = response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");

    let json = extract_json(&text)?;

    let body_text = json
        .get("body_text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ClaudeError::Parse("missing body_text in response".to_string()))?
        .to_string();

    debug!("draft response composed");

    Ok(DraftResponse { body_text })
}

/// Askama template for the response composition user prompt.
#[derive(Template)]
#[template(path = "prompts/compose_reply.txt")]
struct ComposeReplyPrompt<'a> {
    classification: String,
    confidence: String,
    order_numbers: &'a [String],
    product_names: &'a [String],
    from_address: &'a str,
    from_name: Option<&'a str>,
    subject: &'a str,
    body: String,
    shopify_context: Option<&'a str>,
}

/// Build the user prompt for response composition.
fn build_response_prompt(context: &ResponseContext) -> String {
    let template = ComposeReplyPrompt {
        classification: context.classification.classification.to_string(),
        confidence: format!("{:.0}", context.classification.confidence * 100.0),
        order_numbers: &context.classification.extracted_entities.order_numbers,
        product_names: &context.classification.extracted_entities.product_names,
        from_address: &context.from_address,
        from_name: context.from_name.as_deref(),
        subject: &context.subject,
        body: truncate_with_ellipsis(&context.body, 2000),
        shopify_context: context.shopify_context.as_deref(),
    };

    template.render().expect("compose reply prompt template")
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

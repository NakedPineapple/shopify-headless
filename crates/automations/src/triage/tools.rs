//! Claude tool definitions for the email triage pipeline.
//!
//! Defines the `classify_email` tool that instructs Claude to return a
//! structured classification with confidence and extracted entities.

use naked_pineapple_services::claude::Tool;
use serde_json::json;

/// Build the `classify_email` tool definition for the classification step.
#[must_use]
pub fn classify_email_tool() -> Tool {
    Tool {
        name: "classify_email".to_string(),
        description: "Classify an inbound customer email into a category and extract \
            key entities. You MUST call this tool exactly once with your classification."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "required": ["classification", "confidence", "reasoning"],
            "properties": {
                "classification": {
                    "type": "string",
                    "enum": [
                        "spam",
                        "marketing_newsletter",
                        "order_inquiry",
                        "product_question",
                        "return_request",
                        "shipping_issue",
                        "subscription_inquiry",
                        "business_vendor",
                        "complaint",
                        "praise",
                        "unknown"
                    ],
                    "description": "The classification category for this email."
                },
                "sub_category": {
                    "type": "string",
                    "description": "Optional sub-category for more specific classification."
                },
                "confidence": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "description": "Confidence score (0.0-1.0) for this classification."
                },
                "reasoning": {
                    "type": "string",
                    "description": "Brief explanation of why this classification was chosen."
                },
                "order_numbers": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Order numbers mentioned in the email (e.g. '#1234')."
                },
                "product_names": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Product names or SKUs mentioned."
                },
                "tracking_numbers": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Shipping tracking numbers mentioned."
                },
                "customer_name": {
                    "type": "string",
                    "description": "Customer's name if identifiable from signature or body."
                }
            }
        }),
        domain: Some("email_triage".to_string()),
        requires_confirmation: false,
    }
}

/// Build the `compose_reply` tool definition for the response drafting step.
#[must_use]
pub fn compose_reply_tool() -> Tool {
    Tool {
        name: "compose_reply".to_string(),
        description: "Compose a professional, friendly reply to the customer email. \
            The reply should be warm, on-brand for a tropical-luxe e-commerce store, \
            and directly address the customer's question or concern. \
            You MUST call this tool exactly once with your draft reply."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "required": ["subject", "body_html", "body_text"],
            "properties": {
                "subject": {
                    "type": "string",
                    "description": "Reply subject line (usually Re: original subject)."
                },
                "body_html": {
                    "type": "string",
                    "description": "HTML formatted reply body."
                },
                "body_text": {
                    "type": "string",
                    "description": "Plain text version of the reply body."
                }
            }
        }),
        domain: Some("email_triage".to_string()),
        requires_confirmation: false,
    }
}

/// System prompt for the email classification step.
#[must_use]
pub fn classification_system_prompt() -> String {
    "You are an email triage assistant for Naked Pineapple, a tropical-luxe e-commerce \
    brand selling swimwear, resort wear, and accessories. Your job is to classify \
    inbound customer emails into the correct category.\n\n\
    Classification guidelines:\n\
    - spam: Unsolicited messages, phishing, irrelevant content\n\
    - marketing_newsletter: Automated marketing emails, newsletters from other companies\n\
    - order_inquiry: Questions about existing orders, order status, order changes\n\
    - product_question: Questions about products, sizing, materials, availability\n\
    - return_request: Requests to return, exchange, or get a refund\n\
    - shipping_issue: Problems with delivery, lost packages, wrong address\n\
    - subscription_inquiry: Questions about subscriptions, recurring orders, cancellation\n\
    - business_vendor: Wholesale inquiries, partnership proposals, vendor communications\n\
    - complaint: Negative feedback, dissatisfaction, escalation requests\n\
    - praise: Positive feedback, thank you messages, reviews\n\
    - unknown: Cannot determine with confidence\n\n\
    Be precise with confidence scores:\n\
    - 0.9-1.0: Very clear classification (e.g., obvious spam, explicit order number reference)\n\
    - 0.7-0.89: Likely correct but some ambiguity\n\
    - 0.5-0.69: Best guess, significant ambiguity\n\
    - Below 0.5: Use 'unknown' classification\n\n\
    Extract all relevant entities (order numbers, product names, tracking numbers).\n\
    You MUST call the classify_email tool exactly once."
        .to_string()
}

/// System prompt for the response drafting step.
#[must_use]
pub fn response_system_prompt() -> String {
    "You are a customer service representative for Naked Pineapple, a tropical-luxe \
    e-commerce brand. You write warm, professional, and helpful responses to customer \
    emails.\n\n\
    Brand voice guidelines:\n\
    - Friendly and warm, like chatting at a beach resort\n\
    - Professional but never corporate or stiff\n\
    - Empathetic and solution-oriented\n\
    - Use the customer's name when available\n\
    - Keep responses concise but thorough\n\n\
    For the HTML body, use simple formatting (paragraphs, bold for emphasis). \
    Do not include complex HTML or images.\n\n\
    You MUST call the compose_reply tool exactly once with your draft response."
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_email_tool_has_required_fields() {
        let tool = classify_email_tool();
        assert_eq!(tool.name, "classify_email");
        assert!(!tool.requires_confirmation);

        let required = tool
            .input_schema
            .get("required")
            .and_then(|v| v.as_array())
            .expect("required array");
        assert!(required.iter().any(|v| v == "classification"));
        assert!(required.iter().any(|v| v == "confidence"));
        assert!(required.iter().any(|v| v == "reasoning"));
    }

    #[test]
    fn test_compose_reply_tool_has_required_fields() {
        let tool = compose_reply_tool();
        assert_eq!(tool.name, "compose_reply");

        let required = tool
            .input_schema
            .get("required")
            .and_then(|v| v.as_array())
            .expect("required array");
        assert!(required.iter().any(|v| v == "body_html"));
        assert!(required.iter().any(|v| v == "body_text"));
    }
}

//! Human handoff tool.

use naked_pineapple_services::claude::types::Tool;

/// Tool definition for `request_human_help`.
#[must_use]
pub fn tool_definition() -> Tool {
    Tool {
        name: "request_human_help".to_string(),
        description: "Escalate the conversation to a human support agent. Use this when you \
            cannot adequately help the customer, when they are frustrated, or when the issue \
            requires human judgment (complex returns, billing disputes, etc.). If the customer \
            is not logged in, explain that they need to log in or create an account first so \
            the team can follow up."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "reason": {
                    "type": "string",
                    "description": "Brief summary of why human help is needed"
                },
                "category": {
                    "type": "string",
                    "enum": ["order_issue", "return_request", "billing", "product_question", "complaint", "other"],
                    "description": "Category of the support request"
                }
            },
            "required": ["reason", "category"]
        }),
        domain: None,
        requires_confirmation: false,
    }
}

//! FAQ/knowledge base lookup tool.

use naked_pineapple_services::claude::types::Tool;

/// Tool definition for `lookup_faq`.
#[must_use]
pub fn tool_definition() -> Tool {
    Tool {
        name: "lookup_faq".to_string(),
        description: "Search the knowledge base and FAQ content for information about store \
            policies, shipping, returns, ingredients, sizing, and other common questions. \
            Use this when a customer asks about something not covered in the initial context."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to find relevant FAQ/knowledge content"
                }
            },
            "required": ["query"]
        }),
        domain: None,
        requires_confirmation: false,
    }
}

//! Product lookup tool.

use naked_pineapple_services::claude::types::Tool;

/// Tool definition for `lookup_product`.
#[must_use]
pub fn tool_definition() -> Tool {
    Tool {
        name: "lookup_product".to_string(),
        description: "Look up product information including description, ingredients, pricing, \
            and availability. Use this when a customer asks about a specific product."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Product name or search term"
                }
            },
            "required": ["query"]
        }),
        domain: None,
        requires_confirmation: false,
    }
}

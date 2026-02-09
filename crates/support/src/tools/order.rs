//! Order status lookup tool (authenticated only).

use naked_pineapple_services::claude::types::Tool;

/// Tool definition for `lookup_order_status`.
#[must_use]
pub fn tool_definition() -> Tool {
    Tool {
        name: "lookup_order_status".to_string(),
        description: "Check the status of a customer's order including tracking information, \
            fulfillment status, and estimated delivery. Only available for logged-in customers."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "order_number": {
                    "type": "string",
                    "description": "The order number (e.g., '#1001' or '1001')"
                }
            },
            "required": ["order_number"]
        }),
        domain: None,
        requires_confirmation: false,
    }
}

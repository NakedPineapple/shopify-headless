//! Subscription details lookup tool (authenticated only).

use naked_pineapple_services::claude::types::Tool;

/// Tool definition for `lookup_subscription`.
#[must_use]
pub fn tool_definition() -> Tool {
    Tool {
        name: "lookup_subscription".to_string(),
        description: "Check a customer's subscription details including status, next delivery \
            date, and subscription items. Only available for logged-in customers."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "subscription_id": {
                    "type": "string",
                    "description": "The subscription ID, or 'all' to list all active subscriptions"
                }
            },
            "required": ["subscription_id"]
        }),
        domain: None,
        requires_confirmation: false,
    }
}

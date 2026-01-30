//! Gift card tools for Claude.

use serde_json::json;

use crate::claude::types::Tool;

/// Get all gift card-related tools.
#[must_use]
pub fn gift_card_tools() -> Vec<Tool> {
    let mut tools = gift_card_read_tools();
    tools.extend(gift_card_write_tools());
    tools
}

/// Get gift card read-only tools.
fn gift_card_read_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "get_gift_cards_low_level_shopify".to_string(),
            description: "Get gift cards from the store.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Number of gift cards to fetch (default 10)"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query to filter gift cards"
                    }
                }
            }),
            domain: Some("gift_cards".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_gift_cards_count_low_level_shopify".to_string(),
            description: "Get the total count of gift cards.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            domain: Some("gift_cards".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_gift_card_detail_low_level_shopify".to_string(),
            description: "Get detailed information about a specific gift card.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The gift card ID"
                    }
                },
                "required": ["id"]
            }),
            domain: Some("gift_cards".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_gift_card_configuration_low_level_shopify".to_string(),
            description: "Get the store's gift card configuration settings.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            domain: Some("gift_cards".to_string()),
            requires_confirmation: false,
        },
    ]
}

/// Get gift card write tools (require confirmation).
fn gift_card_write_tools() -> Vec<Tool> {
    vec![
        gift_card_create_tool(),
        gift_card_deactivate_tool(),
        gift_card_update_tool(),
        gift_card_credit_tool(),
        gift_card_debit_tool(),
        gift_card_notify_customer_tool(),
        gift_card_notify_recipient_tool(),
    ]
}

fn gift_card_create_tool() -> Tool {
    Tool {
        name: "create_gift_card_low_level_shopify".to_string(),
        description: "Create a new gift card.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "initial_value": {
                    "type": "string",
                    "description": "Initial balance (e.g., '50.00')"
                },
                "code": {
                    "type": "string",
                    "description": "Gift card code (auto-generated if not provided)"
                },
                "customer_id": {
                    "type": "string",
                    "description": "Customer to associate with the gift card"
                },
                "expires_on": {
                    "type": "string",
                    "description": "Expiration date (ISO 8601)"
                },
                "note": {
                    "type": "string",
                    "description": "Internal note"
                }
            },
            "required": ["initial_value"]
        }),
        domain: Some("gift_cards".to_string()),
        requires_confirmation: true,
    }
}

fn gift_card_deactivate_tool() -> Tool {
    Tool {
        name: "deactivate_gift_card_low_level_shopify".to_string(),
        description: "Deactivate a gift card so it can no longer be used.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The gift card ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("gift_cards".to_string()),
        requires_confirmation: true,
    }
}

fn gift_card_update_tool() -> Tool {
    Tool {
        name: "update_gift_card_low_level_shopify".to_string(),
        description: "Update a gift card's expiration date or note.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The gift card ID"
                },
                "expires_on": {
                    "type": "string",
                    "description": "New expiration date (ISO 8601)"
                },
                "note": {
                    "type": "string",
                    "description": "New internal note"
                }
            },
            "required": ["id"]
        }),
        domain: Some("gift_cards".to_string()),
        requires_confirmation: true,
    }
}

fn gift_card_credit_tool() -> Tool {
    Tool {
        name: "credit_gift_card_low_level_shopify".to_string(),
        description: "Add balance to a gift card.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The gift card ID"
                },
                "amount": {
                    "type": "string",
                    "description": "Amount to add (e.g., '25.00')"
                }
            },
            "required": ["id", "amount"]
        }),
        domain: Some("gift_cards".to_string()),
        requires_confirmation: true,
    }
}

fn gift_card_debit_tool() -> Tool {
    Tool {
        name: "debit_gift_card_low_level_shopify".to_string(),
        description: "Remove balance from a gift card.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The gift card ID"
                },
                "amount": {
                    "type": "string",
                    "description": "Amount to remove (e.g., '10.00')"
                }
            },
            "required": ["id", "amount"]
        }),
        domain: Some("gift_cards".to_string()),
        requires_confirmation: true,
    }
}

fn gift_card_notify_customer_tool() -> Tool {
    Tool {
        name: "send_gift_card_notification_to_customer_low_level_shopify".to_string(),
        description: "Send the gift card details to the associated customer via email.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The gift card ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("gift_cards".to_string()),
        requires_confirmation: true,
    }
}

fn gift_card_notify_recipient_tool() -> Tool {
    Tool {
        name: "send_gift_card_notification_to_recipient_low_level_shopify".to_string(),
        description: "Send the gift card details to a specified recipient email.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The gift card ID"
                },
                "recipient_email": {
                    "type": "string",
                    "description": "Email address to send to"
                }
            },
            "required": ["id", "recipient_email"]
        }),
        domain: Some("gift_cards".to_string()),
        requires_confirmation: true,
    }
}

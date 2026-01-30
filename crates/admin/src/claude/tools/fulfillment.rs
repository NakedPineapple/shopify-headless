//! Fulfillment tools for Claude.

use serde_json::json;

use crate::claude::types::Tool;

/// Get all fulfillment-related tools.
#[must_use]
pub fn fulfillment_tools() -> Vec<Tool> {
    let mut tools = fulfillment_read_tools();
    tools.extend(fulfillment_write_tools());
    tools
}

/// Get fulfillment read-only tools.
fn fulfillment_read_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "get_fulfillment_orders".to_string(),
            description: "Get fulfillment orders for an order.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "order_id": {
                        "type": "string",
                        "description": "The order ID"
                    }
                },
                "required": ["order_id"]
            }),
            domain: Some("fulfillment".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_suggested_refund".to_string(),
            description: "Get suggested refund amounts for an order (items, shipping, etc.)."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "order_id": {
                        "type": "string",
                        "description": "The order ID"
                    }
                },
                "required": ["order_id"]
            }),
            domain: Some("fulfillment".to_string()),
            requires_confirmation: false,
        },
    ]
}

/// Get fulfillment write tools (require confirmation).
fn fulfillment_write_tools() -> Vec<Tool> {
    vec![
        fulfillment_create_tool(),
        fulfillment_update_tracking_tool(),
        fulfillment_hold_tool(),
        fulfillment_release_hold_tool(),
        refund_create_tool(),
        return_create_tool(),
    ]
}

fn fulfillment_create_tool() -> Tool {
    Tool {
        name: "create_fulfillment".to_string(),
        description: "Create a fulfillment for order line items. Marks items as shipped."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "fulfillment_order_id": {
                    "type": "string",
                    "description": "The fulfillment order ID"
                },
                "line_items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "quantity": { "type": "integer" }
                        },
                        "required": ["id", "quantity"]
                    },
                    "description": "Line items to fulfill with quantities"
                },
                "tracking_number": {
                    "type": "string",
                    "description": "Tracking number"
                },
                "tracking_url": {
                    "type": "string",
                    "description": "Tracking URL"
                },
                "tracking_company": {
                    "type": "string",
                    "description": "Shipping carrier name"
                },
                "notify_customer": {
                    "type": "boolean",
                    "description": "Send shipment notification to customer"
                }
            },
            "required": ["fulfillment_order_id"]
        }),
        domain: Some("fulfillment".to_string()),
        requires_confirmation: true,
    }
}

fn fulfillment_update_tracking_tool() -> Tool {
    Tool {
        name: "update_fulfillment_tracking".to_string(),
        description: "Update tracking information for an existing fulfillment.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "fulfillment_id": {
                    "type": "string",
                    "description": "The fulfillment ID"
                },
                "tracking_number": {
                    "type": "string",
                    "description": "New tracking number"
                },
                "tracking_url": {
                    "type": "string",
                    "description": "New tracking URL"
                },
                "tracking_company": {
                    "type": "string",
                    "description": "New shipping carrier name"
                },
                "notify_customer": {
                    "type": "boolean",
                    "description": "Notify customer of tracking update"
                }
            },
            "required": ["fulfillment_id"]
        }),
        domain: Some("fulfillment".to_string()),
        requires_confirmation: true,
    }
}

fn fulfillment_hold_tool() -> Tool {
    Tool {
        name: "hold_fulfillment_order".to_string(),
        description: "Place a fulfillment order on hold (prevents fulfillment).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The fulfillment order ID"
                },
                "reason": {
                    "type": "string",
                    "enum": ["AWAITING_PAYMENT", "HIGH_RISK_OF_FRAUD", "INCORRECT_ADDRESS", "INVENTORY_OUT_OF_STOCK", "OTHER"],
                    "description": "Reason for the hold"
                },
                "reason_notes": {
                    "type": "string",
                    "description": "Additional notes about the hold"
                }
            },
            "required": ["id", "reason"]
        }),
        domain: Some("fulfillment".to_string()),
        requires_confirmation: true,
    }
}

fn fulfillment_release_hold_tool() -> Tool {
    Tool {
        name: "release_fulfillment_order_hold".to_string(),
        description: "Release a fulfillment order from hold.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The fulfillment order ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("fulfillment".to_string()),
        requires_confirmation: true,
    }
}

fn refund_create_tool() -> Tool {
    Tool {
        name: "create_refund".to_string(),
        description: "Create a refund for an order.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "order_id": {
                    "type": "string",
                    "description": "The order ID"
                },
                "refund_line_items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "line_item_id": { "type": "string" },
                            "quantity": { "type": "integer" },
                            "restock_type": {
                                "type": "string",
                                "enum": ["NO_RESTOCK", "CANCEL", "RETURN"]
                            }
                        },
                        "required": ["line_item_id", "quantity"]
                    },
                    "description": "Line items to refund"
                },
                "shipping_refund_amount": {
                    "type": "string",
                    "description": "Amount to refund for shipping"
                },
                "note": {
                    "type": "string",
                    "description": "Refund note"
                },
                "notify_customer": {
                    "type": "boolean",
                    "description": "Send refund notification to customer"
                }
            },
            "required": ["order_id"]
        }),
        domain: Some("fulfillment".to_string()),
        requires_confirmation: true,
    }
}

fn return_create_tool() -> Tool {
    Tool {
        name: "create_return".to_string(),
        description: "Create a return for an order (for items being sent back).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "order_id": {
                    "type": "string",
                    "description": "The order ID"
                },
                "return_line_items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "fulfillment_line_item_id": { "type": "string" },
                            "quantity": { "type": "integer" },
                            "return_reason": {
                                "type": "string",
                                "enum": ["COLOR", "DEFECTIVE", "NOT_AS_DESCRIBED", "OTHER", "SIZE_TOO_LARGE", "SIZE_TOO_SMALL", "STYLE", "UNWANTED", "WRONG_ITEM"]
                            }
                        },
                        "required": ["fulfillment_line_item_id", "quantity"]
                    },
                    "description": "Line items to return"
                },
                "notify_customer": {
                    "type": "boolean",
                    "description": "Send return notification to customer"
                }
            },
            "required": ["order_id", "return_line_items"]
        }),
        domain: Some("fulfillment".to_string()),
        requires_confirmation: true,
    }
}

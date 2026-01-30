//! Order tools for Claude.

use serde_json::json;

use crate::claude::types::Tool;

/// Get all order-related tools.
#[must_use]
pub fn order_tools() -> Vec<Tool> {
    let mut tools = order_read_tools();
    tools.extend(order_write_tools());
    tools
}

/// Get order read-only tools.
fn order_read_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "get_order_low_level_shopify".to_string(),
            description: "Get a single order by ID. Returns full order details including \
                customer info, line items, totals, fulfillment status, and payment status."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The order ID (e.g., 'gid://shopify/Order/123' or just '123')"
                    }
                },
                "required": ["id"]
            }),
            domain: Some("orders".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_orders_low_level_shopify".to_string(),
            description: "Get recent orders from the store. Returns order summaries including \
                customer info, line items, totals, and fulfillment status. Use query parameter \
                to filter by status, email, date, etc."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Number of orders to fetch (1-50, default 10)",
                        "minimum": 1,
                        "maximum": 50
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query to filter orders (e.g., 'fulfillment_status:unfulfilled', 'email:customer@example.com', 'created_at:>2024-01-01')"
                    }
                }
            }),
            domain: Some("orders".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_order_detail_low_level_shopify".to_string(),
            description: "Get detailed order information including full line item details, \
                customer addresses, transactions, fulfillments, and refunds."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The order ID"
                    }
                },
                "required": ["id"]
            }),
            domain: Some("orders".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_orders_list_low_level_shopify".to_string(),
            description: "Get a paginated list of orders with sorting. Supports cursor-based \
                pagination for browsing large result sets."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "first": {
                        "type": "integer",
                        "description": "Number of orders per page (1-50, default 10)"
                    },
                    "after": {
                        "type": "string",
                        "description": "Cursor for pagination (from previous response)"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query to filter orders"
                    },
                    "sort_key": {
                        "type": "string",
                        "enum": ["CREATED_AT", "UPDATED_AT", "PROCESSED_AT", "TOTAL_PRICE", "ID"],
                        "description": "Field to sort by"
                    },
                    "reverse": {
                        "type": "boolean",
                        "description": "Reverse sort order (true for descending)"
                    }
                }
            }),
            domain: Some("orders".to_string()),
            requires_confirmation: false,
        },
    ]
}

/// Get order write tools (require confirmation).
fn order_write_tools() -> Vec<Tool> {
    vec![
        order_note_tool(),
        order_tags_tool(),
        order_mark_paid_tool(),
        order_cancel_tool(),
        order_archive_tool(),
        order_unarchive_tool(),
        order_capture_payment_tool(),
        order_add_tags_tool(),
        order_remove_tags_tool(),
    ]
}

fn order_note_tool() -> Tool {
    Tool {
        name: "update_order_note_low_level_shopify".to_string(),
        description: "Update the internal note on an order. Notes are only visible to staff."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "The order ID" },
                "note": { "type": "string", "description": "The new note text (can be empty to clear)" }
            },
            "required": ["id", "note"]
        }),
        domain: Some("orders".to_string()),
        requires_confirmation: true,
    }
}

fn order_tags_tool() -> Tool {
    Tool {
        name: "update_order_tags_low_level_shopify".to_string(),
        description: "Replace all tags on an order. Tags help organize and filter orders."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "The order ID" },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of tags to set (replaces existing tags)"
                }
            },
            "required": ["id", "tags"]
        }),
        domain: Some("orders".to_string()),
        requires_confirmation: true,
    }
}

fn order_mark_paid_tool() -> Tool {
    Tool {
        name: "mark_order_as_paid_low_level_shopify".to_string(),
        description: "Mark an order as paid. Use for manual payment methods like cash or check."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "The order ID" }
            },
            "required": ["id"]
        }),
        domain: Some("orders".to_string()),
        requires_confirmation: true,
    }
}

fn order_cancel_tool() -> Tool {
    Tool {
        name: "cancel_order_low_level_shopify".to_string(),
        description: "Cancel an order. Can optionally refund the customer, restock items, \
            and send notification email."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "The order ID" },
                "reason": {
                    "type": "string",
                    "enum": ["CUSTOMER", "FRAUD", "INVENTORY", "DECLINED", "OTHER"],
                    "description": "Reason for cancellation"
                },
                "refund": { "type": "boolean", "description": "Whether to refund the customer" },
                "restock": { "type": "boolean", "description": "Whether to restock cancelled items" },
                "notify_customer": { "type": "boolean", "description": "Whether to send cancellation email" }
            },
            "required": ["id"]
        }),
        domain: Some("orders".to_string()),
        requires_confirmation: true,
    }
}

fn order_archive_tool() -> Tool {
    Tool {
        name: "archive_order_low_level_shopify".to_string(),
        description: "Archive an order. Archived orders are hidden from the default order list."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "The order ID" }
            },
            "required": ["id"]
        }),
        domain: Some("orders".to_string()),
        requires_confirmation: true,
    }
}

fn order_unarchive_tool() -> Tool {
    Tool {
        name: "unarchive_order_low_level_shopify".to_string(),
        description: "Unarchive an order to restore it to the active order list.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "The order ID" }
            },
            "required": ["id"]
        }),
        domain: Some("orders".to_string()),
        requires_confirmation: true,
    }
}

fn order_capture_payment_tool() -> Tool {
    Tool {
        name: "capture_order_payment_low_level_shopify".to_string(),
        description: "Capture payment for an order that was authorized but not captured. \
            Can capture full or partial amount."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "The order ID" },
                "amount": { "type": "string", "description": "Amount to capture (captures full amount if not specified)" }
            },
            "required": ["id"]
        }),
        domain: Some("orders".to_string()),
        requires_confirmation: true,
    }
}

fn order_add_tags_tool() -> Tool {
    Tool {
        name: "add_tags_to_order_low_level_shopify".to_string(),
        description: "Add tags to an order without removing existing tags.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "The order ID" },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags to add"
                }
            },
            "required": ["id", "tags"]
        }),
        domain: Some("orders".to_string()),
        requires_confirmation: true,
    }
}

fn order_remove_tags_tool() -> Tool {
    Tool {
        name: "remove_tags_from_order_low_level_shopify".to_string(),
        description: "Remove specific tags from an order.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "The order ID" },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags to remove"
                }
            },
            "required": ["id", "tags"]
        }),
        domain: Some("orders".to_string()),
        requires_confirmation: true,
    }
}

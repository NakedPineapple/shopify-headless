//! Order editing tools for Claude.

use serde_json::json;

use crate::claude::types::Tool;

/// Get all order editing-related tools.
#[must_use]
pub fn order_editing_tools() -> Vec<Tool> {
    let mut tools = order_editing_read_tools();
    tools.extend(order_editing_write_tools());
    tools
}

/// Get order editing read-only tools (begin editing session).
fn order_editing_read_tools() -> Vec<Tool> {
    vec![Tool {
        name: "order_edit_begin".to_string(),
        description: "Begin an order editing session. Returns a calculated order with \
            editable fields. Must call order_edit_commit to apply changes."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "order_id": {
                    "type": "string",
                    "description": "The order ID to edit"
                }
            },
            "required": ["order_id"]
        }),
        domain: Some("order_editing".to_string()),
        requires_confirmation: false,
    }]
}

/// Get order editing write tools (require confirmation).
fn order_editing_write_tools() -> Vec<Tool> {
    vec![
        order_edit_add_variant_tool(),
        order_edit_add_custom_item_tool(),
        order_edit_set_quantity_tool(),
        order_edit_add_line_item_discount_tool(),
        order_edit_update_discount_tool(),
        order_edit_remove_discount_tool(),
        order_edit_add_shipping_line_tool(),
        order_edit_update_shipping_line_tool(),
        order_edit_remove_shipping_line_tool(),
        order_edit_commit_tool(),
    ]
}

fn order_edit_add_variant_tool() -> Tool {
    Tool {
        name: "order_edit_add_variant".to_string(),
        description: "Add a product variant to an order being edited.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "calculated_order_id": {
                    "type": "string",
                    "description": "The calculated order ID from order_edit_begin"
                },
                "variant_id": {
                    "type": "string",
                    "description": "The product variant ID to add"
                },
                "quantity": {
                    "type": "integer",
                    "description": "Quantity to add (default 1)"
                }
            },
            "required": ["calculated_order_id", "variant_id"]
        }),
        domain: Some("order_editing".to_string()),
        requires_confirmation: true,
    }
}

fn order_edit_add_custom_item_tool() -> Tool {
    Tool {
        name: "order_edit_add_custom_item".to_string(),
        description: "Add a custom line item (not from catalog) to an order.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "calculated_order_id": {
                    "type": "string",
                    "description": "The calculated order ID"
                },
                "title": {
                    "type": "string",
                    "description": "Title of the custom item"
                },
                "price": {
                    "type": "string",
                    "description": "Price per unit"
                },
                "quantity": {
                    "type": "integer",
                    "description": "Quantity (default 1)"
                },
                "requires_shipping": {
                    "type": "boolean",
                    "description": "Whether item requires shipping"
                },
                "taxable": {
                    "type": "boolean",
                    "description": "Whether item is taxable"
                }
            },
            "required": ["calculated_order_id", "title", "price"]
        }),
        domain: Some("order_editing".to_string()),
        requires_confirmation: true,
    }
}

fn order_edit_set_quantity_tool() -> Tool {
    Tool {
        name: "order_edit_set_quantity".to_string(),
        description: "Change the quantity of a line item in an order.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "calculated_order_id": {
                    "type": "string",
                    "description": "The calculated order ID"
                },
                "line_item_id": {
                    "type": "string",
                    "description": "The line item ID to modify"
                },
                "quantity": {
                    "type": "integer",
                    "description": "New quantity (0 to remove)"
                }
            },
            "required": ["calculated_order_id", "line_item_id", "quantity"]
        }),
        domain: Some("order_editing".to_string()),
        requires_confirmation: true,
    }
}

fn order_edit_add_line_item_discount_tool() -> Tool {
    Tool {
        name: "order_edit_add_line_item_discount".to_string(),
        description: "Add a discount to a specific line item.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "calculated_order_id": {
                    "type": "string",
                    "description": "The calculated order ID"
                },
                "line_item_id": {
                    "type": "string",
                    "description": "The line item ID"
                },
                "description": {
                    "type": "string",
                    "description": "Discount description"
                },
                "percentage": {
                    "type": "number",
                    "description": "Percentage off (0.0-1.0)"
                },
                "fixed_amount": {
                    "type": "string",
                    "description": "Fixed amount off (alternative to percentage)"
                }
            },
            "required": ["calculated_order_id", "line_item_id"]
        }),
        domain: Some("order_editing".to_string()),
        requires_confirmation: true,
    }
}

fn order_edit_update_discount_tool() -> Tool {
    Tool {
        name: "order_edit_update_discount".to_string(),
        description: "Update an existing discount on an order.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "calculated_order_id": {
                    "type": "string",
                    "description": "The calculated order ID"
                },
                "discount_id": {
                    "type": "string",
                    "description": "The discount ID to update"
                },
                "description": {
                    "type": "string",
                    "description": "New description"
                },
                "percentage": {
                    "type": "number",
                    "description": "New percentage"
                },
                "fixed_amount": {
                    "type": "string",
                    "description": "New fixed amount"
                }
            },
            "required": ["calculated_order_id", "discount_id"]
        }),
        domain: Some("order_editing".to_string()),
        requires_confirmation: true,
    }
}

fn order_edit_remove_discount_tool() -> Tool {
    Tool {
        name: "order_edit_remove_discount".to_string(),
        description: "Remove a discount from an order.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "calculated_order_id": {
                    "type": "string",
                    "description": "The calculated order ID"
                },
                "discount_id": {
                    "type": "string",
                    "description": "The discount ID to remove"
                }
            },
            "required": ["calculated_order_id", "discount_id"]
        }),
        domain: Some("order_editing".to_string()),
        requires_confirmation: true,
    }
}

fn order_edit_add_shipping_line_tool() -> Tool {
    Tool {
        name: "order_edit_add_shipping_line".to_string(),
        description: "Add a shipping line to an order.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "calculated_order_id": {
                    "type": "string",
                    "description": "The calculated order ID"
                },
                "title": {
                    "type": "string",
                    "description": "Shipping method title"
                },
                "price": {
                    "type": "string",
                    "description": "Shipping price"
                }
            },
            "required": ["calculated_order_id", "title", "price"]
        }),
        domain: Some("order_editing".to_string()),
        requires_confirmation: true,
    }
}

fn order_edit_update_shipping_line_tool() -> Tool {
    Tool {
        name: "order_edit_update_shipping_line".to_string(),
        description: "Update a shipping line on an order.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "calculated_order_id": {
                    "type": "string",
                    "description": "The calculated order ID"
                },
                "shipping_line_id": {
                    "type": "string",
                    "description": "The shipping line ID"
                },
                "title": {
                    "type": "string",
                    "description": "New title"
                },
                "price": {
                    "type": "string",
                    "description": "New price"
                }
            },
            "required": ["calculated_order_id", "shipping_line_id"]
        }),
        domain: Some("order_editing".to_string()),
        requires_confirmation: true,
    }
}

fn order_edit_remove_shipping_line_tool() -> Tool {
    Tool {
        name: "order_edit_remove_shipping_line".to_string(),
        description: "Remove a shipping line from an order.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "calculated_order_id": {
                    "type": "string",
                    "description": "The calculated order ID"
                },
                "shipping_line_id": {
                    "type": "string",
                    "description": "The shipping line ID to remove"
                }
            },
            "required": ["calculated_order_id", "shipping_line_id"]
        }),
        domain: Some("order_editing".to_string()),
        requires_confirmation: true,
    }
}

fn order_edit_commit_tool() -> Tool {
    Tool {
        name: "order_edit_commit".to_string(),
        description: "Commit all changes to an order. This finalizes the edit session \
            and applies all modifications to the actual order."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "calculated_order_id": {
                    "type": "string",
                    "description": "The calculated order ID"
                },
                "notify_customer": {
                    "type": "boolean",
                    "description": "Send order updated notification to customer"
                },
                "staff_note": {
                    "type": "string",
                    "description": "Internal note about the changes"
                }
            },
            "required": ["calculated_order_id"]
        }),
        domain: Some("order_editing".to_string()),
        requires_confirmation: true,
    }
}

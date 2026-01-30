//! Inventory tools for Claude.

use serde_json::json;

use crate::claude::types::Tool;

/// Get all inventory-related tools.
#[must_use]
pub fn inventory_tools() -> Vec<Tool> {
    let mut tools = inventory_read_tools();
    tools.extend(inventory_write_tools());
    tools
}

/// Get inventory read-only tools.
fn inventory_read_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "get_locations_low_level_shopify".to_string(),
            description: "Get all inventory locations (warehouses, stores) for the shop."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            domain: Some("inventory".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_inventory_levels_low_level_shopify".to_string(),
            description: "Get inventory levels at a location. Returns available, on-hand, \
                and incoming quantities for each item."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "location_id": {
                        "type": "string",
                        "description": "The location ID (uses primary location if not specified)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Number of items to fetch (default 20)",
                        "minimum": 1,
                        "maximum": 50
                    }
                }
            }),
            domain: Some("inventory".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_inventory_items_low_level_shopify".to_string(),
            description: "Get inventory items with their tracking information.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Number of items to fetch (default 20)"
                    }
                }
            }),
            domain: Some("inventory".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_inventory_item_low_level_shopify".to_string(),
            description: "Get a specific inventory item by ID.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The inventory item ID"
                    }
                },
                "required": ["id"]
            }),
            domain: Some("inventory".to_string()),
            requires_confirmation: false,
        },
    ]
}

/// Get inventory write tools (require confirmation).
fn inventory_write_tools() -> Vec<Tool> {
    vec![
        inventory_adjust_tool(),
        inventory_set_tool(),
        inventory_item_update_tool(),
        inventory_move_tool(),
        inventory_activate_tool(),
        inventory_deactivate_tool(),
    ]
}

fn inventory_adjust_tool() -> Tool {
    Tool {
        name: "adjust_inventory_low_level_shopify".to_string(),
        description: "Adjust inventory quantity by a delta (positive or negative). \
            Use for inventory corrections or manual adjustments."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "inventory_item_id": {
                    "type": "string",
                    "description": "The inventory item ID"
                },
                "location_id": {
                    "type": "string",
                    "description": "The location ID"
                },
                "delta": {
                    "type": "integer",
                    "description": "Quantity change (positive to add, negative to remove)"
                },
                "reason": {
                    "type": "string",
                    "description": "Reason for adjustment"
                }
            },
            "required": ["inventory_item_id", "location_id", "delta"]
        }),
        domain: Some("inventory".to_string()),
        requires_confirmation: true,
    }
}

fn inventory_set_tool() -> Tool {
    Tool {
        name: "set_inventory_low_level_shopify".to_string(),
        description: "Set inventory to an absolute quantity. Use for stock counts.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "inventory_item_id": {
                    "type": "string",
                    "description": "The inventory item ID"
                },
                "location_id": {
                    "type": "string",
                    "description": "The location ID"
                },
                "quantity": {
                    "type": "integer",
                    "description": "The new absolute quantity"
                },
                "reason": {
                    "type": "string",
                    "description": "Reason for setting quantity"
                }
            },
            "required": ["inventory_item_id", "location_id", "quantity"]
        }),
        domain: Some("inventory".to_string()),
        requires_confirmation: true,
    }
}

fn inventory_item_update_tool() -> Tool {
    Tool {
        name: "update_inventory_item_low_level_shopify".to_string(),
        description: "Update inventory item properties like cost, country of origin, etc."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The inventory item ID"
                },
                "cost": {
                    "type": "string",
                    "description": "Unit cost"
                },
                "country_code_of_origin": {
                    "type": "string",
                    "description": "Country of origin code (e.g., 'US', 'CN')"
                },
                "harmonized_system_code": {
                    "type": "string",
                    "description": "HS code for customs"
                },
                "tracked": {
                    "type": "boolean",
                    "description": "Whether to track inventory for this item"
                }
            },
            "required": ["id"]
        }),
        domain: Some("inventory".to_string()),
        requires_confirmation: true,
    }
}

fn inventory_move_tool() -> Tool {
    Tool {
        name: "move_inventory_low_level_shopify".to_string(),
        description: "Move inventory between locations (transfer stock).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "inventory_item_id": {
                    "type": "string",
                    "description": "The inventory item ID"
                },
                "from_location_id": {
                    "type": "string",
                    "description": "Source location ID"
                },
                "to_location_id": {
                    "type": "string",
                    "description": "Destination location ID"
                },
                "quantity": {
                    "type": "integer",
                    "description": "Quantity to move"
                }
            },
            "required": ["inventory_item_id", "from_location_id", "to_location_id", "quantity"]
        }),
        domain: Some("inventory".to_string()),
        requires_confirmation: true,
    }
}

fn inventory_activate_tool() -> Tool {
    Tool {
        name: "activate_inventory_low_level_shopify".to_string(),
        description: "Activate inventory tracking at a location for an item.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "inventory_item_id": {
                    "type": "string",
                    "description": "The inventory item ID"
                },
                "location_id": {
                    "type": "string",
                    "description": "The location ID"
                }
            },
            "required": ["inventory_item_id", "location_id"]
        }),
        domain: Some("inventory".to_string()),
        requires_confirmation: true,
    }
}

fn inventory_deactivate_tool() -> Tool {
    Tool {
        name: "deactivate_inventory_low_level_shopify".to_string(),
        description: "Deactivate inventory tracking at a location for an item.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "inventory_item_id": {
                    "type": "string",
                    "description": "The inventory item ID"
                },
                "location_id": {
                    "type": "string",
                    "description": "The location ID"
                }
            },
            "required": ["inventory_item_id", "location_id"]
        }),
        domain: Some("inventory".to_string()),
        requires_confirmation: true,
    }
}

//! Discount tools for Claude.

use serde_json::json;

use crate::claude::types::Tool;

/// Get all discount-related tools.
#[must_use]
pub fn discount_tools() -> Vec<Tool> {
    let mut tools = discount_read_tools();
    tools.extend(discount_write_tools());
    tools
}

/// Get discount read-only tools.
fn discount_read_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "get_discounts".to_string(),
            description: "Get discount codes from the store.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Number of discounts to fetch (default 10)"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query to filter discounts"
                    }
                }
            }),
            domain: Some("discounts".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_discount".to_string(),
            description: "Get a specific discount by ID.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The discount ID"
                    }
                },
                "required": ["id"]
            }),
            domain: Some("discounts".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_discounts_for_list".to_string(),
            description: "Get discounts with pagination for listing/browsing.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "first": {
                        "type": "integer",
                        "description": "Number per page"
                    },
                    "after": {
                        "type": "string",
                        "description": "Pagination cursor"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    }
                }
            }),
            domain: Some("discounts".to_string()),
            requires_confirmation: false,
        },
    ]
}

/// Get discount write tools (require confirmation).
fn discount_write_tools() -> Vec<Tool> {
    vec![
        discount_create_tool(),
        discount_update_tool(),
        discount_deactivate_tool(),
        discount_activate_tool(),
        discount_deactivate_automatic_tool(),
        discount_delete_tool(),
        discount_bulk_activate_tool(),
        discount_bulk_deactivate_tool(),
        discount_bulk_delete_tool(),
    ]
}

fn discount_create_tool() -> Tool {
    Tool {
        name: "create_discount".to_string(),
        description: "Create a new discount code.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Internal discount title"
                },
                "code": {
                    "type": "string",
                    "description": "Customer-facing discount code"
                },
                "percentage": {
                    "type": "number",
                    "description": "Percentage off (0.0-1.0, e.g., 0.2 for 20% off)"
                },
                "amount": {
                    "type": "string",
                    "description": "Fixed amount off (mutually exclusive with percentage)"
                },
                "currency_code": {
                    "type": "string",
                    "description": "Currency for fixed amount (e.g., 'USD')"
                },
                "starts_at": {
                    "type": "string",
                    "description": "Start date (ISO 8601)"
                },
                "ends_at": {
                    "type": "string",
                    "description": "End date (ISO 8601, optional)"
                },
                "usage_limit": {
                    "type": "integer",
                    "description": "Max total uses (optional)"
                }
            },
            "required": ["title", "code", "starts_at"]
        }),
        domain: Some("discounts".to_string()),
        requires_confirmation: true,
    }
}

fn discount_update_tool() -> Tool {
    Tool {
        name: "update_discount".to_string(),
        description: "Update an existing discount.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The discount ID"
                },
                "title": {
                    "type": "string",
                    "description": "New title"
                },
                "starts_at": {
                    "type": "string",
                    "description": "New start date"
                },
                "ends_at": {
                    "type": "string",
                    "description": "New end date"
                }
            },
            "required": ["id"]
        }),
        domain: Some("discounts".to_string()),
        requires_confirmation: true,
    }
}

fn discount_deactivate_tool() -> Tool {
    Tool {
        name: "deactivate_discount".to_string(),
        description: "Deactivate a discount code (can be reactivated later).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The discount ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("discounts".to_string()),
        requires_confirmation: true,
    }
}

fn discount_activate_tool() -> Tool {
    Tool {
        name: "activate_discount".to_string(),
        description: "Reactivate a previously deactivated discount.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The discount ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("discounts".to_string()),
        requires_confirmation: true,
    }
}

fn discount_deactivate_automatic_tool() -> Tool {
    Tool {
        name: "deactivate_automatic_discount".to_string(),
        description: "Deactivate an automatic discount.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The automatic discount ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("discounts".to_string()),
        requires_confirmation: true,
    }
}

fn discount_delete_tool() -> Tool {
    Tool {
        name: "delete_discount".to_string(),
        description: "Permanently delete a discount.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The discount ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("discounts".to_string()),
        requires_confirmation: true,
    }
}

fn discount_bulk_activate_tool() -> Tool {
    Tool {
        name: "bulk_activate_code_discounts".to_string(),
        description: "Activate multiple discount codes at once.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Discount IDs to activate"
                }
            },
            "required": ["ids"]
        }),
        domain: Some("discounts".to_string()),
        requires_confirmation: true,
    }
}

fn discount_bulk_deactivate_tool() -> Tool {
    Tool {
        name: "bulk_deactivate_code_discounts".to_string(),
        description: "Deactivate multiple discount codes at once.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Discount IDs to deactivate"
                }
            },
            "required": ["ids"]
        }),
        domain: Some("discounts".to_string()),
        requires_confirmation: true,
    }
}

fn discount_bulk_delete_tool() -> Tool {
    Tool {
        name: "bulk_delete_code_discounts".to_string(),
        description: "Delete multiple discount codes at once.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Discount IDs to delete"
                }
            },
            "required": ["ids"]
        }),
        domain: Some("discounts".to_string()),
        requires_confirmation: true,
    }
}

//! Customer tools for Claude.

use serde_json::json;

use crate::claude::types::Tool;

/// Get all customer-related tools.
#[must_use]
pub fn customer_tools() -> Vec<Tool> {
    let mut tools = customer_read_tools();
    tools.extend(customer_write_tools());
    tools
}

/// Get customer read-only tools.
fn customer_read_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "get_customer".to_string(),
            description: "Get a single customer by ID. Returns customer details including \
                name, email, addresses, order history, and marketing preferences."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The customer ID"
                    }
                },
                "required": ["id"]
            }),
            domain: Some("customers".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_customers".to_string(),
            description: "Get customers from the store. Returns customer summaries including \
                name, email, order count, and total spent. Use query to filter by email, name, etc."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Number of customers to fetch (1-50, default 10)",
                        "minimum": 1,
                        "maximum": 50
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query to filter customers (e.g., 'email:john@example.com', 'orders_count:>5')"
                    },
                    "sort_key": {
                        "type": "string",
                        "enum": ["NAME", "CREATED_AT", "UPDATED_AT", "ORDERS_COUNT", "TOTAL_SPENT"],
                        "description": "Field to sort by"
                    },
                    "reverse": {
                        "type": "boolean",
                        "description": "Reverse sort order (true for descending)"
                    }
                }
            }),
            domain: Some("customers".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "generate_customer_activation_url".to_string(),
            description: "Generate an account activation URL for a customer. Use when customer \
                needs to set up their account password."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The customer ID"
                    }
                },
                "required": ["id"]
            }),
            domain: Some("customers".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_customer_segments".to_string(),
            description: "Get customer segments (saved searches). Segments group customers \
                based on shared characteristics."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Number of segments to fetch (default 10)"
                    }
                }
            }),
            domain: Some("customers".to_string()),
            requires_confirmation: false,
        },
    ]
}

/// Get customer write tools (require confirmation).
fn customer_write_tools() -> Vec<Tool> {
    vec![
        customer_create_tool(),
        customer_update_tool(),
        customer_delete_tool(),
        customer_add_tags_tool(),
        customer_remove_tags_tool(),
        customer_send_invite_tool(),
        customer_create_address_tool(),
        customer_update_address_tool(),
        customer_delete_address_tool(),
        customer_set_default_address_tool(),
        customer_update_email_marketing_tool(),
        customer_update_sms_marketing_tool(),
        customer_merge_tool(),
    ]
}

fn customer_create_tool() -> Tool {
    Tool {
        name: "create_customer".to_string(),
        description: "Create a new customer account.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "email": {
                    "type": "string",
                    "description": "Customer email address"
                },
                "first_name": {
                    "type": "string",
                    "description": "Customer first name"
                },
                "last_name": {
                    "type": "string",
                    "description": "Customer last name"
                },
                "phone": {
                    "type": "string",
                    "description": "Customer phone number"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags to apply to the customer"
                },
                "note": {
                    "type": "string",
                    "description": "Internal note about the customer"
                },
                "accepts_marketing": {
                    "type": "boolean",
                    "description": "Whether customer accepts marketing emails"
                }
            },
            "required": ["email"]
        }),
        domain: Some("customers".to_string()),
        requires_confirmation: true,
    }
}

fn customer_update_tool() -> Tool {
    Tool {
        name: "update_customer".to_string(),
        description: "Update an existing customer's information.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The customer ID"
                },
                "email": {
                    "type": "string",
                    "description": "New email address"
                },
                "first_name": {
                    "type": "string",
                    "description": "New first name"
                },
                "last_name": {
                    "type": "string",
                    "description": "New last name"
                },
                "phone": {
                    "type": "string",
                    "description": "New phone number"
                },
                "note": {
                    "type": "string",
                    "description": "New internal note"
                }
            },
            "required": ["id"]
        }),
        domain: Some("customers".to_string()),
        requires_confirmation: true,
    }
}

fn customer_delete_tool() -> Tool {
    Tool {
        name: "delete_customer".to_string(),
        description: "Delete a customer. This action cannot be undone.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The customer ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("customers".to_string()),
        requires_confirmation: true,
    }
}

fn customer_add_tags_tool() -> Tool {
    Tool {
        name: "add_customer_tags".to_string(),
        description: "Add tags to a customer without removing existing tags.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The customer ID"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags to add"
                }
            },
            "required": ["id", "tags"]
        }),
        domain: Some("customers".to_string()),
        requires_confirmation: true,
    }
}

fn customer_remove_tags_tool() -> Tool {
    Tool {
        name: "remove_customer_tags".to_string(),
        description: "Remove specific tags from a customer.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The customer ID"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags to remove"
                }
            },
            "required": ["id", "tags"]
        }),
        domain: Some("customers".to_string()),
        requires_confirmation: true,
    }
}

fn customer_send_invite_tool() -> Tool {
    Tool {
        name: "send_customer_invite".to_string(),
        description: "Send an account invitation email to a customer.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The customer ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("customers".to_string()),
        requires_confirmation: true,
    }
}

fn customer_create_address_tool() -> Tool {
    Tool {
        name: "create_customer_address".to_string(),
        description: "Add a new address to a customer's address book.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "customer_id": {
                    "type": "string",
                    "description": "The customer ID"
                },
                "address1": {
                    "type": "string",
                    "description": "Street address line 1"
                },
                "address2": {
                    "type": "string",
                    "description": "Street address line 2"
                },
                "city": {
                    "type": "string",
                    "description": "City"
                },
                "province": {
                    "type": "string",
                    "description": "Province/State code"
                },
                "country": {
                    "type": "string",
                    "description": "Country code (e.g., 'US', 'CA')"
                },
                "zip": {
                    "type": "string",
                    "description": "Postal/ZIP code"
                },
                "phone": {
                    "type": "string",
                    "description": "Phone number for the address"
                }
            },
            "required": ["customer_id", "address1", "city", "country"]
        }),
        domain: Some("customers".to_string()),
        requires_confirmation: true,
    }
}

fn customer_update_address_tool() -> Tool {
    Tool {
        name: "update_customer_address".to_string(),
        description: "Update an existing customer address.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "customer_id": {
                    "type": "string",
                    "description": "The customer ID"
                },
                "address_id": {
                    "type": "string",
                    "description": "The address ID to update"
                },
                "address1": { "type": "string" },
                "address2": { "type": "string" },
                "city": { "type": "string" },
                "province": { "type": "string" },
                "country": { "type": "string" },
                "zip": { "type": "string" },
                "phone": { "type": "string" }
            },
            "required": ["customer_id", "address_id"]
        }),
        domain: Some("customers".to_string()),
        requires_confirmation: true,
    }
}

fn customer_delete_address_tool() -> Tool {
    Tool {
        name: "delete_customer_address".to_string(),
        description: "Delete an address from a customer's address book.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "customer_id": {
                    "type": "string",
                    "description": "The customer ID"
                },
                "address_id": {
                    "type": "string",
                    "description": "The address ID to delete"
                }
            },
            "required": ["customer_id", "address_id"]
        }),
        domain: Some("customers".to_string()),
        requires_confirmation: true,
    }
}

fn customer_set_default_address_tool() -> Tool {
    Tool {
        name: "set_customer_default_address".to_string(),
        description: "Set a customer's default shipping address.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "customer_id": {
                    "type": "string",
                    "description": "The customer ID"
                },
                "address_id": {
                    "type": "string",
                    "description": "The address ID to set as default"
                }
            },
            "required": ["customer_id", "address_id"]
        }),
        domain: Some("customers".to_string()),
        requires_confirmation: true,
    }
}

fn customer_update_email_marketing_tool() -> Tool {
    Tool {
        name: "update_customer_email_marketing".to_string(),
        description: "Update a customer's email marketing consent status.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The customer ID"
                },
                "marketing_state": {
                    "type": "string",
                    "enum": ["SUBSCRIBED", "UNSUBSCRIBED", "NOT_SUBSCRIBED", "PENDING"],
                    "description": "The new marketing consent state"
                },
                "marketing_opt_in_level": {
                    "type": "string",
                    "enum": ["SINGLE_OPT_IN", "CONFIRMED_OPT_IN"],
                    "description": "Level of consent (optional)"
                }
            },
            "required": ["id", "marketing_state"]
        }),
        domain: Some("customers".to_string()),
        requires_confirmation: true,
    }
}

fn customer_update_sms_marketing_tool() -> Tool {
    Tool {
        name: "update_customer_sms_marketing".to_string(),
        description: "Update a customer's SMS marketing consent status.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The customer ID"
                },
                "marketing_state": {
                    "type": "string",
                    "enum": ["SUBSCRIBED", "UNSUBSCRIBED", "NOT_SUBSCRIBED", "PENDING"],
                    "description": "The new SMS marketing consent state"
                },
                "marketing_opt_in_level": {
                    "type": "string",
                    "enum": ["SINGLE_OPT_IN", "CONFIRMED_OPT_IN"],
                    "description": "Level of consent (optional)"
                }
            },
            "required": ["id", "marketing_state"]
        }),
        domain: Some("customers".to_string()),
        requires_confirmation: true,
    }
}

fn customer_merge_tool() -> Tool {
    Tool {
        name: "merge_customers".to_string(),
        description: "Merge two customer records into one. The source customer's data \
            will be merged into the destination customer."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "source_customer_id": {
                    "type": "string",
                    "description": "The customer ID to merge from (will be deleted)"
                },
                "destination_customer_id": {
                    "type": "string",
                    "description": "The customer ID to merge into (will be kept)"
                }
            },
            "required": ["source_customer_id", "destination_customer_id"]
        }),
        domain: Some("customers".to_string()),
        requires_confirmation: true,
    }
}

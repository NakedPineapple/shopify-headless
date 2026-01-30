//! Collection tools for Claude.

use serde_json::json;

use crate::claude::types::Tool;

/// Get all collection-related tools.
#[must_use]
pub fn collection_tools() -> Vec<Tool> {
    let mut tools = collection_read_tools();
    tools.extend(collection_write_tools());
    tools
}

/// Get collection read-only tools.
fn collection_read_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "get_collection_low_level_shopify".to_string(),
            description: "Get a single collection by ID.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The collection ID"
                    }
                },
                "required": ["id"]
            }),
            domain: Some("collections".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_collections_low_level_shopify".to_string(),
            description: "Get collections from the store. Returns collection summaries."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Number of collections to fetch (default 10)"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query to filter collections"
                    }
                }
            }),
            domain: Some("collections".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_collection_with_products_low_level_shopify".to_string(),
            description: "Get a collection with its products.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The collection ID"
                    },
                    "product_limit": {
                        "type": "integer",
                        "description": "Max products to return (default 20)"
                    }
                },
                "required": ["id"]
            }),
            domain: Some("collections".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_publications_low_level_shopify".to_string(),
            description: "Get all sales channels/publications where collections can be published."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            domain: Some("collections".to_string()),
            requires_confirmation: false,
        },
    ]
}

/// Get collection write tools (require confirmation).
fn collection_write_tools() -> Vec<Tool> {
    vec![
        collection_create_tool(),
        collection_update_tool(),
        collection_sort_order_tool(),
        collection_delete_tool(),
        collection_image_update_tool(),
        collection_image_delete_tool(),
        collection_add_products_tool(),
        collection_remove_products_tool(),
        collection_reorder_products_tool(),
        collection_publish_tool(),
        collection_unpublish_tool(),
    ]
}

fn collection_create_tool() -> Tool {
    Tool {
        name: "create_collection_low_level_shopify".to_string(),
        description: "Create a new collection.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Collection title"
                },
                "description_html": {
                    "type": "string",
                    "description": "Collection description (HTML)"
                },
                "sort_order": {
                    "type": "string",
                    "enum": ["MANUAL", "BEST_SELLING", "ALPHA_ASC", "ALPHA_DESC", "PRICE_ASC", "PRICE_DESC", "CREATED", "CREATED_DESC"],
                    "description": "How products are sorted in the collection"
                }
            },
            "required": ["title"]
        }),
        domain: Some("collections".to_string()),
        requires_confirmation: true,
    }
}

fn collection_update_tool() -> Tool {
    Tool {
        name: "update_collection_low_level_shopify".to_string(),
        description: "Update an existing collection's details.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The collection ID"
                },
                "title": {
                    "type": "string",
                    "description": "New title"
                },
                "description_html": {
                    "type": "string",
                    "description": "New description (HTML)"
                }
            },
            "required": ["id"]
        }),
        domain: Some("collections".to_string()),
        requires_confirmation: true,
    }
}

fn collection_sort_order_tool() -> Tool {
    Tool {
        name: "update_collection_sort_order_low_level_shopify".to_string(),
        description: "Change how products are sorted in a collection.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The collection ID"
                },
                "sort_order": {
                    "type": "string",
                    "enum": ["MANUAL", "BEST_SELLING", "ALPHA_ASC", "ALPHA_DESC", "PRICE_ASC", "PRICE_DESC", "CREATED", "CREATED_DESC"],
                    "description": "New sort order"
                }
            },
            "required": ["id", "sort_order"]
        }),
        domain: Some("collections".to_string()),
        requires_confirmation: true,
    }
}

fn collection_delete_tool() -> Tool {
    Tool {
        name: "delete_collection_low_level_shopify".to_string(),
        description: "Delete a collection. Products remain but are no longer grouped.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The collection ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("collections".to_string()),
        requires_confirmation: true,
    }
}

fn collection_image_update_tool() -> Tool {
    Tool {
        name: "update_collection_image_low_level_shopify".to_string(),
        description: "Set or update a collection's featured image.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The collection ID"
                },
                "image_url": {
                    "type": "string",
                    "description": "URL of the image to set"
                },
                "alt_text": {
                    "type": "string",
                    "description": "Alt text for accessibility"
                }
            },
            "required": ["id", "image_url"]
        }),
        domain: Some("collections".to_string()),
        requires_confirmation: true,
    }
}

fn collection_image_delete_tool() -> Tool {
    Tool {
        name: "delete_collection_image_low_level_shopify".to_string(),
        description: "Remove a collection's featured image.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The collection ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("collections".to_string()),
        requires_confirmation: true,
    }
}

fn collection_add_products_tool() -> Tool {
    Tool {
        name: "add_products_to_collection_low_level_shopify".to_string(),
        description: "Add products to a manual collection.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The collection ID"
                },
                "product_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Product IDs to add"
                }
            },
            "required": ["id", "product_ids"]
        }),
        domain: Some("collections".to_string()),
        requires_confirmation: true,
    }
}

fn collection_remove_products_tool() -> Tool {
    Tool {
        name: "remove_products_from_collection_low_level_shopify".to_string(),
        description: "Remove products from a manual collection.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The collection ID"
                },
                "product_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Product IDs to remove"
                }
            },
            "required": ["id", "product_ids"]
        }),
        domain: Some("collections".to_string()),
        requires_confirmation: true,
    }
}

fn collection_reorder_products_tool() -> Tool {
    Tool {
        name: "reorder_collection_products_low_level_shopify".to_string(),
        description: "Reorder products in a manual collection.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The collection ID"
                },
                "moves": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "product_id": { "type": "string" },
                            "new_position": { "type": "integer" }
                        },
                        "required": ["product_id", "new_position"]
                    },
                    "description": "Products to move with their new positions"
                }
            },
            "required": ["id", "moves"]
        }),
        domain: Some("collections".to_string()),
        requires_confirmation: true,
    }
}

fn collection_publish_tool() -> Tool {
    Tool {
        name: "publish_collection_low_level_shopify".to_string(),
        description: "Publish a collection to a sales channel.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The collection ID"
                },
                "publication_id": {
                    "type": "string",
                    "description": "The publication/channel ID"
                }
            },
            "required": ["id", "publication_id"]
        }),
        domain: Some("collections".to_string()),
        requires_confirmation: true,
    }
}

fn collection_unpublish_tool() -> Tool {
    Tool {
        name: "unpublish_collection_low_level_shopify".to_string(),
        description: "Remove a collection from a sales channel.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The collection ID"
                },
                "publication_id": {
                    "type": "string",
                    "description": "The publication/channel ID"
                }
            },
            "required": ["id", "publication_id"]
        }),
        domain: Some("collections".to_string()),
        requires_confirmation: true,
    }
}

//! Product tools for Claude.

use serde_json::json;

use crate::claude::types::Tool;

/// Get all product-related tools.
#[must_use]
pub fn product_tools() -> Vec<Tool> {
    let mut tools = product_read_tools();
    tools.extend(product_write_tools());
    tools
}

/// Get product read-only tools.
fn product_read_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "get_product_low_level_shopify".to_string(),
            description: "Get a single product by ID. Returns product details including \
                title, description, variants, pricing, inventory, and media."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The product ID"
                    }
                },
                "required": ["id"]
            }),
            domain: Some("products".to_string()),
            requires_confirmation: false,
        },
        Tool {
            name: "get_products_low_level_shopify".to_string(),
            description: "Get products from the store. Returns product summaries including \
                title, status, variants, and inventory levels."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Number of products to fetch (1-50, default 10)",
                        "minimum": 1,
                        "maximum": 50
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query to filter products (e.g., 'title:Moisturizer', 'status:active', 'vendor:BrandName')"
                    }
                }
            }),
            domain: Some("products".to_string()),
            requires_confirmation: false,
        },
    ]
}

/// Get product write tools (require confirmation).
fn product_write_tools() -> Vec<Tool> {
    vec![
        product_create_tool(),
        product_update_tool(),
        product_delete_tool(),
        variant_update_tool(),
    ]
}

fn product_create_tool() -> Tool {
    Tool {
        name: "create_product_low_level_shopify".to_string(),
        description: "Create a new product in the store.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Product title"
                },
                "description_html": {
                    "type": "string",
                    "description": "Product description (HTML)"
                },
                "vendor": {
                    "type": "string",
                    "description": "Product vendor/brand"
                },
                "product_type": {
                    "type": "string",
                    "description": "Product type/category"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Product tags"
                },
                "status": {
                    "type": "string",
                    "enum": ["ACTIVE", "DRAFT", "ARCHIVED"],
                    "description": "Product status"
                },
                "variants": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "price": { "type": "string" },
                            "sku": { "type": "string" },
                            "inventory_quantity": { "type": "integer" }
                        }
                    },
                    "description": "Product variants"
                }
            },
            "required": ["title"]
        }),
        domain: Some("products".to_string()),
        requires_confirmation: true,
    }
}

fn product_update_tool() -> Tool {
    Tool {
        name: "update_product_low_level_shopify".to_string(),
        description: "Update an existing product. Only provided fields will be changed."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The product ID"
                },
                "title": {
                    "type": "string",
                    "description": "New product title"
                },
                "description_html": {
                    "type": "string",
                    "description": "New product description (HTML)"
                },
                "vendor": {
                    "type": "string",
                    "description": "New vendor/brand"
                },
                "product_type": {
                    "type": "string",
                    "description": "New product type"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "New tags (replaces existing)"
                },
                "status": {
                    "type": "string",
                    "enum": ["ACTIVE", "DRAFT", "ARCHIVED"],
                    "description": "New status"
                }
            },
            "required": ["id"]
        }),
        domain: Some("products".to_string()),
        requires_confirmation: true,
    }
}

fn product_delete_tool() -> Tool {
    Tool {
        name: "delete_product_low_level_shopify".to_string(),
        description: "Delete a product from the store. This action cannot be undone.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The product ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("products".to_string()),
        requires_confirmation: true,
    }
}

fn variant_update_tool() -> Tool {
    Tool {
        name: "update_variant_low_level_shopify".to_string(),
        description: "Update a product variant's price, SKU, or other attributes.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The variant ID"
                },
                "price": {
                    "type": "string",
                    "description": "New price"
                },
                "compare_at_price": {
                    "type": "string",
                    "description": "New compare-at price (for sale pricing)"
                },
                "sku": {
                    "type": "string",
                    "description": "New SKU"
                },
                "barcode": {
                    "type": "string",
                    "description": "New barcode"
                },
                "weight": {
                    "type": "number",
                    "description": "Weight value"
                },
                "weight_unit": {
                    "type": "string",
                    "enum": ["GRAMS", "KILOGRAMS", "OUNCES", "POUNDS"],
                    "description": "Weight unit"
                }
            },
            "required": ["id"]
        }),
        domain: Some("products".to_string()),
        requires_confirmation: true,
    }
}

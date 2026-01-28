//! Shopify tool definitions and executor for Claude tool use.
//!
//! Provides tools for querying Shopify data (orders, products, customers, inventory)
//! that Claude can use to answer questions about the store.

use serde_json::json;
use tracing::instrument;

use crate::shopify::AdminClient;

use super::error::ClaudeError;
use super::types::Tool;

/// Get the list of Shopify tools available to Claude.
#[must_use]
pub fn shopify_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "get_orders".to_string(),
            description: "Get recent orders from the Shopify store. Returns order details including customer info, line items, totals, and fulfillment status.".to_string(),
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
                        "description": "Optional search query to filter orders (e.g., 'fulfillment_status:unfulfilled', 'email:customer@example.com')"
                    }
                }
            }),
        },
        Tool {
            name: "get_products".to_string(),
            description: "Get products from the Shopify store. Returns product details including title, description, variants, pricing, and inventory.".to_string(),
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
                        "description": "Optional search query to filter products (e.g., 'title:Moisturizer', 'status:active')"
                    }
                }
            }),
        },
        Tool {
            name: "get_customers".to_string(),
            description: "Get customers from the Shopify store. Returns customer details including name, email, order history, and total spent.".to_string(),
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
                        "description": "Optional search query to filter customers (e.g., 'email:john@example.com', 'orders_count:>5')"
                    }
                }
            }),
        },
        Tool {
            name: "get_inventory".to_string(),
            description: "Get inventory levels at a specific location. Returns available, on-hand, and incoming quantities for each inventory item.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "location_id": {
                        "type": "string",
                        "description": "Shopify location ID (e.g., 'gid://shopify/Location/123'). If not provided, uses the primary location."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Number of inventory items to fetch (1-50, default 20)",
                        "minimum": 1,
                        "maximum": 50
                    }
                }
            }),
        },
    ]
}

/// Executor for Shopify tools.
///
/// Handles tool execution by mapping tool names to Shopify Admin API calls.
pub struct ToolExecutor<'a> {
    shopify: &'a AdminClient,
}

impl<'a> ToolExecutor<'a> {
    /// Create a new tool executor.
    #[must_use]
    pub const fn new(shopify: &'a AdminClient) -> Self {
        Self { shopify }
    }

    /// Execute a tool and return the result as a string.
    ///
    /// # Arguments
    ///
    /// * `name` - Tool name
    /// * `input` - Tool input parameters
    ///
    /// # Errors
    ///
    /// Returns an error if the tool execution fails.
    #[instrument(skip(self, input), fields(tool_name = %name))]
    pub async fn execute(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        match name {
            "get_orders" => self.get_orders(input).await,
            "get_products" => self.get_products(input).await,
            "get_customers" => self.get_customers(input).await,
            "get_inventory" => self.get_inventory(input).await,
            _ => Err(ClaudeError::ToolExecution(format!("Unknown tool: {name}"))),
        }
    }

    /// Get orders from Shopify.
    async fn get_orders(&self, input: &serde_json::Value) -> Result<String, ClaudeError> {
        let limit = input["limit"].as_i64().unwrap_or(10).clamp(1, 50);
        let query = input["query"].as_str().map(String::from);

        let result = self
            .shopify
            .get_orders(limit, None, query)
            .await
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get orders: {e}")))?;

        // Summarize the results to minimize tokens
        let summary = summarize_orders(&result.orders);
        serde_json::to_string_pretty(&summary)
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to serialize orders: {e}")))
    }

    /// Get products from Shopify.
    async fn get_products(&self, input: &serde_json::Value) -> Result<String, ClaudeError> {
        let limit = input["limit"].as_i64().unwrap_or(10).clamp(1, 50);
        let query = input["query"].as_str().map(String::from);

        let result = self
            .shopify
            .get_products(limit, None, query)
            .await
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get products: {e}")))?;

        // Summarize the results to minimize tokens
        let summary = summarize_products(&result.products);
        serde_json::to_string_pretty(&summary)
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to serialize products: {e}")))
    }

    /// Get customers from Shopify.
    async fn get_customers(&self, input: &serde_json::Value) -> Result<String, ClaudeError> {
        let limit = input["limit"].as_i64().unwrap_or(10).clamp(1, 50);
        let query = input["query"].as_str().map(String::from);

        let params = crate::shopify::types::CustomerListParams {
            first: Some(limit),
            query,
            ..Default::default()
        };
        let result = self
            .shopify
            .get_customers(params)
            .await
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get customers: {e}")))?;

        // Summarize the results to minimize tokens
        let summary = summarize_customers(&result.customers);
        serde_json::to_string_pretty(&summary)
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to serialize customers: {e}")))
    }

    /// Get inventory levels from Shopify.
    async fn get_inventory(&self, input: &serde_json::Value) -> Result<String, ClaudeError> {
        let location_id = input["location_id"]
            .as_str()
            .unwrap_or("gid://shopify/Location/1"); // Default to primary location
        let limit = input["limit"].as_i64().unwrap_or(20).clamp(1, 50);

        let result = self
            .shopify
            .get_inventory_levels(location_id, limit, None)
            .await
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to get inventory: {e}")))?;

        // Summarize the results to minimize tokens
        let summary = summarize_inventory(&result.inventory_levels);
        serde_json::to_string_pretty(&summary)
            .map_err(|e| ClaudeError::ToolExecution(format!("Failed to serialize inventory: {e}")))
    }
}

/// Summarize orders to minimize token usage.
fn summarize_orders(orders: &[crate::shopify::Order]) -> serde_json::Value {
    let summaries: Vec<serde_json::Value> = orders
        .iter()
        .map(|o| {
            json!({
                "name": o.name,
                "created_at": o.created_at,
                "financial_status": o.financial_status,
                "fulfillment_status": o.fulfillment_status,
                "total_price": format!("{} {}", o.total_price.amount, o.currency_code),
                "email": o.email,
                "line_items_count": o.line_items.len(),
                "fully_paid": o.fully_paid,
            })
        })
        .collect();

    json!({
        "count": summaries.len(),
        "orders": summaries,
    })
}

/// Summarize products to minimize token usage.
fn summarize_products(products: &[crate::shopify::AdminProduct]) -> serde_json::Value {
    let summaries: Vec<serde_json::Value> = products
        .iter()
        .map(|p| {
            let variant_count = p.variants.len();
            let price_range = if p.variants.is_empty() {
                "N/A".to_string()
            } else {
                let prices: Vec<&str> =
                    p.variants.iter().map(|v| v.price.amount.as_str()).collect();
                match (prices.first(), prices.last()) {
                    (Some(first), Some(last)) if first == last => (*first).to_string(),
                    (Some(first), Some(last)) => format!("{first} - {last}"),
                    _ => "N/A".to_string(),
                }
            };

            json!({
                "id": p.id,
                "title": p.title,
                "handle": p.handle,
                "status": p.status,
                "vendor": p.vendor,
                "total_inventory": p.total_inventory,
                "variant_count": variant_count,
                "price_range": price_range,
            })
        })
        .collect();

    json!({
        "count": summaries.len(),
        "products": summaries,
    })
}

/// Summarize customers to minimize token usage.
fn summarize_customers(customers: &[crate::shopify::Customer]) -> serde_json::Value {
    let summaries: Vec<serde_json::Value> = customers
        .iter()
        .map(|c| {
            json!({
                "id": c.id,
                "display_name": c.display_name,
                "email": c.email,
                "orders_count": c.orders_count,
                "total_spent": format!("{} {}", c.total_spent.amount, c.total_spent.currency_code),
                "state": c.state,
                "accepts_marketing": c.accepts_marketing,
            })
        })
        .collect();

    json!({
        "count": summaries.len(),
        "customers": summaries,
    })
}

/// Summarize inventory levels to minimize token usage.
fn summarize_inventory(levels: &[crate::shopify::InventoryLevel]) -> serde_json::Value {
    let summaries: Vec<serde_json::Value> = levels
        .iter()
        .map(|l| {
            json!({
                "inventory_item_id": l.inventory_item_id,
                "location": l.location_name,
                "available": l.available,
                "on_hand": l.on_hand,
                "incoming": l.incoming,
            })
        })
        .collect();

    json!({
        "count": summaries.len(),
        "inventory_levels": summaries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shopify_tools_count() {
        let tools = shopify_tools();
        assert_eq!(tools.len(), 4);
    }

    #[test]
    fn test_shopify_tools_names() {
        let tools = shopify_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

        assert!(names.contains(&"get_orders"));
        assert!(names.contains(&"get_products"));
        assert!(names.contains(&"get_customers"));
        assert!(names.contains(&"get_inventory"));
    }

    #[test]
    fn test_tool_input_schema_is_object() {
        let tools = shopify_tools();
        for tool in tools {
            assert_eq!(tool.input_schema.get("type"), Some(&json!("object")));
        }
    }
}

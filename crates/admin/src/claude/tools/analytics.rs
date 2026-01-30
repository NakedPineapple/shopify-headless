//! High-level analytics tools for Claude.
//!
//! These tools provide summarized, aggregate data that's more useful for
//! answering business questions like "what's our revenue this month?"
//! without returning verbose low-level API data.
//!
//! Tool naming follows Shopify's analytics report conventions.

use serde_json::json;

use crate::claude::types::Tool;

/// Get all analytics tools (15 total).
#[must_use]
pub fn analytics_tools() -> Vec<Tool> {
    vec![
        // Sales & Revenue (5)
        sales_summary_tool(),
        sales_by_channel_tool(),
        sales_by_product_tool(),
        sales_by_location_tool(),
        sales_by_discount_tool(),
        // Orders (1)
        order_summary_tool(),
        // Customers (3)
        customer_summary_tool(),
        top_customers_tool(),
        customers_by_location_tool(),
        // Products & Inventory (2)
        product_catalog_tool(),
        inventory_summary_tool(),
        // Finance (3)
        profit_summary_tool(),
        payout_summary_tool(),
        gift_card_summary_tool(),
        // Fulfillment (1)
        fulfillment_summary_tool(),
    ]
}

// =============================================================================
// Sales & Revenue Tools
// =============================================================================

fn sales_summary_tool() -> Tool {
    Tool {
        name: "get_sales_summary".to_string(),
        description: "Get a comprehensive sales summary for a date range. Returns total sales, \
            gross sales, net sales (after discounts/returns), total tax collected, total shipping, \
            order count, and average order value (AOV). \
            USE THIS for questions about revenue, sales totals, how much money was made, AOV, \
            or tax/shipping totals."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "start_date": {
                    "type": "string",
                    "description": "Start date in YYYY-MM-DD format (e.g., '2024-01-01')"
                },
                "end_date": {
                    "type": "string",
                    "description": "End date in YYYY-MM-DD format (e.g., '2024-01-31')"
                }
            },
            "required": ["start_date", "end_date"]
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

fn sales_by_channel_tool() -> Tool {
    Tool {
        name: "get_sales_by_channel".to_string(),
        description: "Get sales breakdown by sales channel for a date range. Returns order count \
            and revenue for each channel (Online Store, POS, Shop app, draft orders, etc.). \
            USE THIS for questions about where sales come from, channel performance, or comparing \
            online vs in-store sales."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "start_date": {
                    "type": "string",
                    "description": "Start date in YYYY-MM-DD format"
                },
                "end_date": {
                    "type": "string",
                    "description": "End date in YYYY-MM-DD format"
                }
            },
            "required": ["start_date", "end_date"]
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

fn sales_by_product_tool() -> Tool {
    Tool {
        name: "get_sales_by_product".to_string(),
        description: "Get sales breakdown by product for a date range. Returns top products \
            ranked by revenue or units sold, with product title, variant info, units sold, \
            and revenue per product. \
            USE THIS for questions about best sellers, worst sellers, top products, product \
            performance, or units sold."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "start_date": {
                    "type": "string",
                    "description": "Start date in YYYY-MM-DD format"
                },
                "end_date": {
                    "type": "string",
                    "description": "End date in YYYY-MM-DD format"
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of products to return (default 10, max 50)",
                    "minimum": 1,
                    "maximum": 50
                },
                "sort_by": {
                    "type": "string",
                    "enum": ["revenue", "units_sold"],
                    "description": "Sort by revenue (default) or units_sold"
                }
            },
            "required": ["start_date", "end_date"]
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

fn sales_by_location_tool() -> Tool {
    Tool {
        name: "get_sales_by_location".to_string(),
        description: "Get sales breakdown by customer location for a date range. Returns order \
            count and revenue grouped by country and optionally by state/province. \
            USE THIS for questions about where customers are located, geographic sales \
            distribution, or sales by country/region."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "start_date": {
                    "type": "string",
                    "description": "Start date in YYYY-MM-DD format"
                },
                "end_date": {
                    "type": "string",
                    "description": "End date in YYYY-MM-DD format"
                },
                "group_by": {
                    "type": "string",
                    "enum": ["country", "state"],
                    "description": "Group by country (default) or state/province"
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of locations to return (default 10)",
                    "minimum": 1,
                    "maximum": 50
                }
            },
            "required": ["start_date", "end_date"]
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

fn sales_by_discount_tool() -> Tool {
    Tool {
        name: "get_sales_by_discount".to_string(),
        description: "Get sales breakdown by discount code for a date range. Returns total orders \
            with discounts, total discount amount given, revenue from discounted orders, and \
            top discount codes by usage. \
            USE THIS for questions about discount usage, promo code performance, how much was \
            discounted, or which codes are most popular."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "start_date": {
                    "type": "string",
                    "description": "Start date in YYYY-MM-DD format"
                },
                "end_date": {
                    "type": "string",
                    "description": "End date in YYYY-MM-DD format"
                },
                "code": {
                    "type": "string",
                    "description": "Filter to a specific discount code (optional)"
                }
            },
            "required": ["start_date", "end_date"]
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

// =============================================================================
// Order Tools
// =============================================================================

fn order_summary_tool() -> Tool {
    Tool {
        name: "get_order_summary".to_string(),
        description: "Get order summary for a date range. Returns total order count, breakdown \
            by fulfillment status (unfulfilled, fulfilled, partially fulfilled), breakdown by \
            financial status (paid, pending, refunded, partially refunded), cancellation count \
            and rate, and return count. \
            USE THIS for questions about order volumes, order statuses, cancellation rates, \
            or return rates."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "start_date": {
                    "type": "string",
                    "description": "Start date in YYYY-MM-DD format"
                },
                "end_date": {
                    "type": "string",
                    "description": "End date in YYYY-MM-DD format"
                }
            },
            "required": ["start_date", "end_date"]
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

// =============================================================================
// Customer Tools
// =============================================================================

fn customer_summary_tool() -> Tool {
    Tool {
        name: "get_customer_summary".to_string(),
        description: "Get customer summary for a date range. Returns new customer count (first \
            order in period), returning customer count (ordered before and during period), \
            total unique customers who ordered, and email marketing subscriber count. \
            USE THIS for questions about customer acquisition, new vs returning customers, \
            retention, or marketing subscribers."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "start_date": {
                    "type": "string",
                    "description": "Start date in YYYY-MM-DD format"
                },
                "end_date": {
                    "type": "string",
                    "description": "End date in YYYY-MM-DD format"
                }
            },
            "required": ["start_date", "end_date"]
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

fn top_customers_tool() -> Tool {
    Tool {
        name: "get_top_customers".to_string(),
        description: "Get top customers ranked by total spend or order count. Returns customer \
            name, email (partial), total lifetime spent, order count, and last order date. \
            USE THIS for questions about best customers, top spenders, VIP customers, or \
            customer leaderboard."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Number of top customers to return (default 10, max 50)",
                    "minimum": 1,
                    "maximum": 50
                },
                "sort_by": {
                    "type": "string",
                    "enum": ["total_spent", "order_count"],
                    "description": "Sort by total_spent (default) or order_count"
                }
            }
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

fn customers_by_location_tool() -> Tool {
    Tool {
        name: "get_customers_by_location".to_string(),
        description: "Get customer count breakdown by location. Returns customer counts grouped \
            by country and optionally by state/province. \
            USE THIS for questions about where customers are located or geographic customer \
            distribution."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "group_by": {
                    "type": "string",
                    "enum": ["country", "state"],
                    "description": "Group by country (default) or state/province"
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of locations to return (default 10)",
                    "minimum": 1,
                    "maximum": 50
                }
            }
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

// =============================================================================
// Product & Inventory Tools
// =============================================================================

fn product_catalog_tool() -> Tool {
    Tool {
        name: "get_product_catalog".to_string(),
        description: "Get product catalog information. Returns products with title, handle, \
            price range, status (active/draft/archived), variant count, and total inventory. \
            USE THIS for questions about what products exist, product prices, product status, \
            or browsing the catalog."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["ACTIVE", "DRAFT", "ARCHIVED"],
                    "description": "Filter by product status (default: all)"
                },
                "query": {
                    "type": "string",
                    "description": "Search query to filter products by title, vendor, tag, etc."
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of products to return (default 20, max 50)",
                    "minimum": 1,
                    "maximum": 50
                }
            }
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

fn inventory_summary_tool() -> Tool {
    Tool {
        name: "get_inventory_summary".to_string(),
        description: "Get inventory summary across all products. Returns total SKU count, \
            out of stock count with product list, low stock count with product list, and \
            total inventory value if costs are set. \
            USE THIS for questions about stock levels, what's out of stock, low inventory \
            alerts, or inventory health."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "low_stock_threshold": {
                    "type": "integer",
                    "description": "Threshold for low stock warning (default 10)",
                    "minimum": 1
                }
            }
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

// =============================================================================
// Finance Tools
// =============================================================================

fn profit_summary_tool() -> Tool {
    Tool {
        name: "get_profit_summary".to_string(),
        description: "Get profit summary for a date range. Returns gross sales, cost of goods \
            sold (COGS), gross profit, and profit margin percentage. Requires products to have \
            cost per item set in Shopify. \
            USE THIS for questions about profit, margins, COGS, or profitability."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "start_date": {
                    "type": "string",
                    "description": "Start date in YYYY-MM-DD format"
                },
                "end_date": {
                    "type": "string",
                    "description": "End date in YYYY-MM-DD format"
                }
            },
            "required": ["start_date", "end_date"]
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

fn payout_summary_tool() -> Tool {
    Tool {
        name: "get_payout_summary".to_string(),
        description: "Get payout and financial summary. Returns recent payouts with amounts and \
            dates, next scheduled payout if available, total transaction fees, and open dispute \
            count and amount. \
            USE THIS for questions about payouts, when money arrives, bank deposits, fees, or \
            chargebacks/disputes."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Number of recent payouts to include (default 5, max 20)",
                    "minimum": 1,
                    "maximum": 20
                }
            }
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

fn gift_card_summary_tool() -> Tool {
    Tool {
        name: "get_gift_card_summary".to_string(),
        description: "Get gift card summary. Returns total gift cards issued, total outstanding \
            balance (unredeemed liability), gift cards sold in period with revenue, and \
            disabled/expired gift card count. \
            USE THIS for questions about gift card balances, gift card liability, gift card \
            sales, or outstanding gift cards."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "start_date": {
                    "type": "string",
                    "description": "Start date for gift cards sold count (optional)"
                },
                "end_date": {
                    "type": "string",
                    "description": "End date for gift cards sold count (optional)"
                }
            }
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

// =============================================================================
// Fulfillment Tools
// =============================================================================

fn fulfillment_summary_tool() -> Tool {
    Tool {
        name: "get_fulfillment_summary".to_string(),
        description: "Get fulfillment and shipping summary. Returns orders awaiting fulfillment, \
            orders on hold, partially fulfilled orders, orders fulfilled today, and average \
            time from order to fulfillment. \
            USE THIS for questions about shipping status, pending shipments, fulfillment \
            backlog, or shipping performance."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
        domain: Some("analytics".to_string()),
        requires_confirmation: false,
    }
}

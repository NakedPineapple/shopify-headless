//! `ShopifyQL` analytics for business summary emails.
//!
//! Executes `ShopifyQL` queries against the Shopify Admin API to gather
//! revenue, order, and channel metrics for daily/weekly business summaries.

use rust_decimal::Decimal;
use tracing::{debug, instrument, warn};

use super::client::{ShopifyClient, ShopifyError};

// =============================================================================
// Result Types
// =============================================================================

/// Aggregate sales metrics for a time period.
#[derive(Debug, Clone, Default)]
pub struct SummaryMetrics {
    /// Total gross revenue.
    pub total_revenue: Decimal,
    /// Total number of orders.
    pub total_orders: i64,
    /// Total units sold.
    pub total_units: i64,
    /// Average order value.
    pub average_order_value: Decimal,
}

/// Revenue breakdown for a single product.
#[derive(Debug, Clone)]
pub struct ProductRevenue {
    /// Product title.
    pub title: String,
    /// Total revenue for this product.
    pub revenue: Decimal,
    /// Number of orders containing this product.
    pub orders: i64,
    /// Units sold.
    pub units: i64,
}

/// Revenue breakdown for a single sales channel.
#[derive(Debug, Clone)]
pub struct ChannelMetrics {
    /// Channel name (e.g., "Online Store", "Point of Sale").
    pub channel_name: String,
    /// Total revenue from this channel.
    pub revenue: Decimal,
    /// Number of orders from this channel.
    pub orders: i64,
}

// =============================================================================
// ShopifyQL GraphQL Query
// =============================================================================

const SHOPIFYQL_QUERY: &str = r"
query ShopifyqlQuery($query: String!) {
    shopifyqlQuery(query: $query) {
        parseErrors
        tableData {
            columns {
                name
                dataType
                displayName
            }
            rows
        }
    }
}
";

/// Column metadata from a `ShopifyQL` response.
struct ShopifyqlColumn {
    name: String,
}

/// Parsed `ShopifyQL` result with columns and rows.
struct ShopifyqlResult {
    columns: Vec<ShopifyqlColumn>,
    rows: Vec<serde_json::Value>,
}

impl ShopifyqlResult {
    fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }
}

// =============================================================================
// Query Execution
// =============================================================================

/// Execute a raw `ShopifyQL` query and return parsed results.
async fn execute_shopifyql(
    client: &ShopifyClient,
    query: &str,
) -> Result<ShopifyqlResult, ShopifyError> {
    let variables = serde_json::json!({ "query": query });
    let data = client.graphql(SHOPIFYQL_QUERY, variables).await?;

    let query_response = data
        .get("shopifyqlQuery")
        .ok_or_else(|| ShopifyError::GraphQL("ShopifyQL query returned no response".into()))?;

    // Check for parse errors
    if let Some(errors) = query_response.get("parseErrors").and_then(|e| e.as_array())
        && !errors.is_empty()
    {
        let messages: Vec<&str> = errors.iter().filter_map(|e| e.as_str()).collect();
        return Err(ShopifyError::GraphQL(format!(
            "ShopifyQL parse errors: {}",
            messages.join("; ")
        )));
    }

    let table_data = query_response.get("tableData");

    let columns: Vec<ShopifyqlColumn> = table_data
        .and_then(|td| td.get("columns"))
        .and_then(|c| c.as_array())
        .map(|cols| {
            cols.iter()
                .filter_map(|c| {
                    Some(ShopifyqlColumn {
                        name: c.get("name")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let rows: Vec<serde_json::Value> = table_data
        .and_then(|td| td.get("rows"))
        .and_then(|r| r.as_array().cloned())
        .unwrap_or_default();

    Ok(ShopifyqlResult { columns, rows })
}

// =============================================================================
// Summary Analytics Functions
// =============================================================================

/// Get aggregate sales metrics for a date range.
///
/// Uses `ShopifyQL` `SINCE`/`UNTIL` date literals (e.g., `2026-01-01`).
///
/// # Errors
///
/// Returns `ShopifyError` if the API request or query fails.
#[instrument(skip(client))]
pub async fn get_summary_analytics(
    client: &ShopifyClient,
    since: &str,
    until: &str,
) -> Result<SummaryMetrics, ShopifyError> {
    let query = format!(
        "FROM sales SHOW total_sales, orders, ordered_item_quantity \
         SINCE {since} UNTIL {until}"
    );

    let result = execute_shopifyql(client, &query).await?;

    let total_sales_idx = result.column_index("total_sales");
    let orders_idx = result.column_index("orders");
    let units_idx = result.column_index("ordered_item_quantity");

    let mut metrics = SummaryMetrics::default();

    for row in &result.rows {
        let row_arr = row.as_array();

        let revenue = total_sales_idx
            .and_then(|i| row_arr.and_then(|r| r.get(i)))
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);

        let orders = orders_idx
            .and_then(|i| row_arr.and_then(|r| r.get(i)))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);

        let units = units_idx
            .and_then(|i| row_arr.and_then(|r| r.get(i)))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);

        metrics.total_revenue += Decimal::from_f64_retain(revenue).unwrap_or_default();
        metrics.total_orders += orders;
        metrics.total_units += units;
    }

    if metrics.total_orders > 0 {
        metrics.average_order_value = metrics.total_revenue / Decimal::from(metrics.total_orders);
    }

    debug!(
        revenue = %metrics.total_revenue,
        orders = metrics.total_orders,
        "fetched summary analytics"
    );

    Ok(metrics)
}

/// Get top products by revenue for a date range.
///
/// # Errors
///
/// Returns `ShopifyError` if the API request or query fails.
#[instrument(skip(client))]
pub async fn get_top_products(
    client: &ShopifyClient,
    since: &str,
    until: &str,
    limit: usize,
) -> Result<Vec<ProductRevenue>, ShopifyError> {
    let query = format!(
        "FROM sales SHOW total_sales, orders, ordered_item_quantity \
         GROUP BY product_title SINCE {since} UNTIL {until}"
    );

    let result = execute_shopifyql(client, &query).await?;

    let title_idx = result.column_index("product_title");
    let total_sales_idx = result.column_index("total_sales");
    let orders_idx = result.column_index("orders");
    let units_idx = result.column_index("ordered_item_quantity");

    let mut products: Vec<ProductRevenue> = result
        .rows
        .iter()
        .filter_map(|row| {
            let row_arr = row.as_array()?;

            let title = title_idx
                .and_then(|i| row_arr.get(i))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();

            let revenue = total_sales_idx
                .and_then(|i| row_arr.get(i))
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);

            let orders = orders_idx
                .and_then(|i| row_arr.get(i))
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);

            let units = units_idx
                .and_then(|i| row_arr.get(i))
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);

            Some(ProductRevenue {
                title,
                revenue: Decimal::from_f64_retain(revenue).unwrap_or_default(),
                orders,
                units,
            })
        })
        .collect();

    // Sort by revenue descending and take top N
    products.sort_by(|a, b| b.revenue.cmp(&a.revenue));
    products.truncate(limit);

    debug!(count = products.len(), "fetched top products");
    Ok(products)
}

/// Get sales breakdown by channel for a date range.
///
/// # Errors
///
/// Returns `ShopifyError` if the API request or query fails.
#[instrument(skip(client))]
pub async fn get_channel_breakdown(
    client: &ShopifyClient,
    since: &str,
    until: &str,
) -> Result<Vec<ChannelMetrics>, ShopifyError> {
    let query = format!(
        "FROM sales SHOW total_sales, orders \
         GROUP BY sales_channel SINCE {since} UNTIL {until}"
    );

    let result = execute_shopifyql(client, &query).await?;

    let channel_idx = result.column_index("sales_channel");
    let total_sales_idx = result.column_index("total_sales");
    let orders_idx = result.column_index("orders");

    let mut channels: Vec<ChannelMetrics> = result
        .rows
        .iter()
        .filter_map(|row| {
            let row_arr = row.as_array()?;

            let channel_name = channel_idx
                .and_then(|i| row_arr.get(i))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();

            let revenue = total_sales_idx
                .and_then(|i| row_arr.get(i))
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);

            let orders = orders_idx
                .and_then(|i| row_arr.get(i))
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);

            Some(ChannelMetrics {
                channel_name,
                revenue: Decimal::from_f64_retain(revenue).unwrap_or_default(),
                orders,
            })
        })
        .collect();

    // Sort by revenue descending
    channels.sort_by(|a, b| b.revenue.cmp(&a.revenue));

    debug!(count = channels.len(), "fetched channel breakdown");
    Ok(channels)
}

/// Fetch low stock products from Shopify inventory.
///
/// Returns product titles with inventory below the threshold. This is a thin
/// wrapper around the existing `inventory::fetch_all_inventory` for use in
/// summary emails — the full low stock workflow handles alerting separately.
///
/// # Errors
///
/// Returns `ShopifyError` if the API request fails.
#[instrument(skip(client))]
pub async fn get_low_stock_items(
    client: &ShopifyClient,
    threshold: i32,
) -> Result<Vec<LowStockItem>, ShopifyError> {
    use super::inventory;

    let products = inventory::fetch_all_inventory(client).await?;

    let items: Vec<LowStockItem> = products
        .into_iter()
        .filter(|p| p.total_inventory < threshold && p.total_inventory >= 0)
        .map(|p| LowStockItem {
            title: p.title,
            inventory: p.total_inventory,
        })
        .collect();

    if !items.is_empty() {
        warn!(
            count = items.len(),
            threshold, "found low stock items for summary"
        );
    }

    Ok(items)
}

/// A product below the low stock threshold, for inclusion in summaries.
#[derive(Debug, Clone)]
pub struct LowStockItem {
    /// Product title.
    pub title: String,
    /// Current inventory level.
    pub inventory: i32,
}

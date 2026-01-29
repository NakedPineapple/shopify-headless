//! Analytics operations for the Admin API.
//!
//! Provides methods for executing `ShopifyQL` queries and retrieving
//! channel/marketing analytics data.

use tracing::instrument;

use super::{
    AdminClient, AdminShopifyError,
    queries::{GetSalesChannels, GetSalesChannelsCount, ShopifyqlQuery},
};
use crate::shopify::types::{
    AnalyticsSummary, ChannelMetrics, DailyMetrics, DateRange, SalesChannel, SalesChannelApp,
    ShopifyqlColumn, ShopifyqlResult,
};

impl AdminClient {
    /// Execute a `ShopifyQL` query.
    ///
    /// # Arguments
    ///
    /// * `query` - The `ShopifyQL` query string (e.g., `FROM sales SHOW total_sales SINCE -30d`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or the query has parse errors.
    #[instrument(skip(self))]
    pub async fn execute_shopifyql(
        &self,
        query: &str,
    ) -> Result<ShopifyqlResult, AdminShopifyError> {
        let variables = super::queries::shopifyql_query::Variables {
            query: query.to_string(),
        };

        let response = self.execute::<ShopifyqlQuery>(variables).await?;

        // shopifyql_query may be None if the query failed to execute
        let query_response = response.shopifyql_query.ok_or_else(|| {
            AdminShopifyError::UserError("ShopifyQL query returned no response".to_string())
        })?;

        // Check for parse errors
        if !query_response.parse_errors.is_empty() {
            return Err(AdminShopifyError::UserError(format!(
                "ShopifyQL parse errors: {}",
                query_response.parse_errors.join("; ")
            )));
        }

        // Extract table data
        let table_data = &query_response.table_data;

        let columns: Vec<ShopifyqlColumn> = table_data
            .as_ref()
            .map(|td| {
                td.columns
                    .iter()
                    .map(|c| ShopifyqlColumn {
                        name: c.name.clone(),
                        display_name: c.display_name.clone(),
                        data_type: format!("{:?}", c.data_type),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let rows: Vec<serde_json::Value> = table_data
            .as_ref()
            .and_then(|td| td.rows.as_array().cloned())
            .unwrap_or_default();

        Ok(ShopifyqlResult {
            columns,
            rows,
            parse_errors: query_response.parse_errors,
        })
    }

    /// Get all sales channels (publications).
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[allow(deprecated)] // app and name fields are deprecated but still functional
    #[instrument(skip(self))]
    pub async fn get_sales_channels(&self) -> Result<Vec<SalesChannel>, AdminShopifyError> {
        let mut channels = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let variables = super::queries::get_sales_channels::Variables {
                first: Some(50),
                after: cursor.clone(),
            };

            let response = self.execute::<GetSalesChannels>(variables).await?;

            for edge in response.publications.edges {
                let node = edge.node;

                // App is a direct struct field (deprecated but still works)
                let app = Some(SalesChannelApp {
                    id: node.app.id.clone(),
                    title: node.app.title.clone(),
                    handle: node.app.handle.clone(),
                });

                channels.push(SalesChannel {
                    id: node.id,
                    name: node.name,
                    app,
                    auto_publish: node.auto_publish,
                    supports_future_publishing: node.supports_future_publishing,
                });
            }

            let page_info = response.publications.page_info;
            if page_info.has_next_page {
                cursor = page_info.end_cursor;
            } else {
                break;
            }
        }

        Ok(channels)
    }

    /// Get the count of sales channels.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_sales_channels_count(&self) -> Result<i64, AdminShopifyError> {
        let variables = super::queries::get_sales_channels_count::Variables {};

        let response = self.execute::<GetSalesChannelsCount>(variables).await?;

        // publicationsCount returns a Count object with a count field
        let count = response.publications_count.map_or(0, |c| c.count);

        Ok(count)
    }

    /// Get channel analytics summary for a date range.
    ///
    /// Uses `ShopifyQL` to aggregate sales data by channel.
    ///
    /// # Arguments
    ///
    /// * `date_range` - The date range to query
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    // Order counts in e-commerce will never exceed i64's f64-safe range (2^52)
    #[allow(clippy::cast_precision_loss)]
    #[instrument(skip(self))]
    pub async fn get_channel_analytics(
        &self,
        date_range: &DateRange,
    ) -> Result<AnalyticsSummary, AdminShopifyError> {
        let query = format!(
            "FROM sales SHOW total_sales, net_sales, orders, ordered_item_quantity \
             GROUP BY sales_channel SINCE {} UNTIL {}",
            date_range.start, date_range.end
        );

        let result = self.execute_shopifyql(&query).await?;

        // Find column indices
        let channel_idx = result.column_index("sales_channel");
        let total_sales_idx = result.column_index("total_sales");
        let net_sales_idx = result.column_index("net_sales");
        let orders_idx = result.column_index("orders");
        let units_idx = result.column_index("ordered_item_quantity");

        let mut channels = Vec::new();

        for row in &result.rows {
            let row_arr = row.as_array();

            let channel_name = channel_idx
                .and_then(|i| row_arr.and_then(|r| r.get(i)))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();

            let total_sales = total_sales_idx
                .and_then(|i| row_arr.and_then(|r| r.get(i)))
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);

            let net_sales = net_sales_idx
                .and_then(|i| row_arr.and_then(|r| r.get(i)))
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);

            let orders = orders_idx
                .and_then(|i| row_arr.and_then(|r| r.get(i)))
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);

            let units_sold = units_idx
                .and_then(|i| row_arr.and_then(|r| r.get(i)))
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);

            let average_order_value = if orders > 0 {
                total_sales / orders as f64
            } else {
                0.0
            };

            channels.push(ChannelMetrics {
                channel_name,
                total_sales,
                net_sales,
                orders,
                units_sold,
                average_order_value,
            });
        }

        let mut summary = AnalyticsSummary {
            channels,
            ..Default::default()
        };
        summary.calculate_totals();

        Ok(summary)
    }

    /// Get daily sales trend, optionally filtered by channel.
    ///
    /// # Arguments
    ///
    /// * `channel` - Optional channel name to filter by
    /// * `date_range` - The date range to query
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    #[instrument(skip(self))]
    pub async fn get_channel_trend(
        &self,
        channel: Option<&str>,
        date_range: &DateRange,
    ) -> Result<Vec<DailyMetrics>, AdminShopifyError> {
        let where_clause = channel
            .map(|c| format!(" WHERE sales_channel = '{c}'"))
            .unwrap_or_default();

        let query = format!(
            "FROM sales SHOW total_sales, orders \
             GROUP BY day{} SINCE {} UNTIL {} ORDER BY day ASC",
            where_clause, date_range.start, date_range.end
        );

        let result = self.execute_shopifyql(&query).await?;

        let day_idx = result.column_index("day");
        let total_sales_idx = result.column_index("total_sales");
        let orders_idx = result.column_index("orders");

        let mut metrics = Vec::new();

        for row in &result.rows {
            let row_arr = row.as_array();

            let date = day_idx
                .and_then(|i| row_arr.and_then(|r| r.get(i)))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let total_sales = total_sales_idx
                .and_then(|i| row_arr.and_then(|r| r.get(i)))
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);

            let orders = orders_idx
                .and_then(|i| row_arr.and_then(|r| r.get(i)))
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);

            metrics.push(DailyMetrics {
                date,
                total_sales,
                orders,
                channel_name: channel.map(String::from),
            });
        }

        Ok(metrics)
    }
}

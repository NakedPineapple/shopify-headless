//! Orders list page handler.

use askama::Template;
use axum::{
    extract::{Query, State},
    response::Html,
};
use tracing::instrument;

use crate::{
    components::data_table::{
        BulkAction, FilterType, TableColumn, TableFilter, orders_table_config,
    },
    filters,
    middleware::auth::RequireAdminAuth,
    shopify::types::OrderSortKey,
    state::AppState,
};

use super::super::dashboard::AdminUserView;
use super::types::{
    OrderColumnVisibility, OrderTableView, OrdersQuery, build_preserve_params, build_shopify_query,
};

/// Orders list page template with data table support.
#[derive(Template)]
#[template(path = "orders/index.html")]
pub struct OrdersIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    /// Data table ID.
    pub table_id: String,
    /// Column definitions.
    pub columns: Vec<TableColumn>,
    /// Filter definitions.
    pub filters: Vec<TableFilter>,
    /// Bulk action definitions.
    pub bulk_actions: Vec<BulkAction>,
    /// Default visible columns as JSON array.
    pub default_columns: Vec<String>,
    /// Column visibility state.
    pub col_visible: OrderColumnVisibility,
    /// Orders to display.
    pub orders: Vec<OrderTableView>,
    /// Whether there are more pages.
    pub has_next_page: bool,
    /// Cursor for next page.
    pub next_cursor: Option<String>,
    /// Current search query.
    pub search_value: Option<String>,
    /// Current sort column.
    pub sort_column: Option<String>,
    /// Current sort direction.
    pub sort_direction: String,
    /// Parameters to preserve in pagination links.
    pub preserve_params: String,
    /// Active filter values for highlighting.
    pub filter_values: std::collections::HashMap<String, String>,
}

/// Orders list page handler.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<OrdersQuery>,
) -> Html<String> {
    // Get table configuration
    let config = orders_table_config();

    // Build Shopify query from filters
    let shopify_query = build_shopify_query(&query);

    // Determine sort key and direction
    let sort_key = query
        .sort
        .as_ref()
        .and_then(|s| OrderSortKey::from_str_param(s));
    let reverse = query.dir.as_deref() == Some("desc");

    // Fetch orders using the extended list endpoint
    let result = state
        .shopify()
        .get_orders_list(25, query.cursor.clone(), shopify_query, sort_key, reverse)
        .await;

    let (orders, has_next_page, next_cursor) = match result {
        Ok(conn) => {
            let orders: Vec<OrderTableView> =
                conn.orders.iter().map(OrderTableView::from).collect();
            (
                orders,
                conn.page_info.has_next_page,
                conn.page_info.end_cursor,
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch orders: {e}");
            (vec![], false, None)
        }
    };

    // Build column visibility from defaults
    let default_columns = config.default_columns();
    let col_visible = OrderColumnVisibility::from_columns(&default_columns);

    // Build filter values map for highlighting active filters
    let mut filter_values = std::collections::HashMap::new();
    if let Some(fs) = &query.financial_status {
        filter_values.insert("financial_status".to_string(), fs.clone());
    }
    if let Some(fs) = &query.fulfillment_status {
        filter_values.insert("fulfillment_status".to_string(), fs.clone());
    }
    if let Some(rs) = &query.return_status {
        filter_values.insert("return_status".to_string(), rs.clone());
    }
    if let Some(s) = &query.status {
        filter_values.insert("status".to_string(), s.clone());
    }
    if let Some(rl) = &query.risk_level {
        filter_values.insert("risk_level".to_string(), rl.clone());
    }

    let preserve_params = build_preserve_params(&query);

    let template = OrdersIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/orders".to_string(),
        table_id: config.table_id.clone(),
        columns: config.columns,
        filters: config.filters,
        bulk_actions: config.bulk_actions,
        default_columns,
        col_visible,
        orders,
        has_next_page,
        next_cursor,
        search_value: query.query,
        sort_column: query.sort,
        sort_direction: query.dir.unwrap_or_else(|| "desc".to_string()),
        preserve_params,
        filter_values,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

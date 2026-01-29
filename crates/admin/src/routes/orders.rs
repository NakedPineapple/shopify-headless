//! Orders management route handlers.

use askama::Template;
use axum::{
    Form,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use chrono::Utc;
use serde::Deserialize;
use tracing::instrument;

use crate::{
    components::data_table::{
        BulkAction, FilterType, TableColumn, TableFilter, orders_table_config,
    },
    filters,
    middleware::auth::RequireAdminAuth,
    shopify::types::{
        Address, CalculatedLineItem, CalculatedOrder, CalculatedShippingLine,
        CalculatedShippingLineStagedStatus, DeliveryCategory, FinancialStatus, Fulfillment,
        FulfillmentStatus, Money, Order, OrderEditAddShippingLineInput,
        OrderEditAppliedDiscountInput, OrderEditUpdateShippingLineInput, OrderLineItem,
        OrderListItem, OrderReturnStatus, OrderRiskLevel, OrderSortKey,
    },
    state::AppState,
};

use super::dashboard::AdminUserView;

// =============================================================================
// Query Parameters
// =============================================================================

/// Query parameters for orders list with filtering, sorting, and pagination.
#[derive(Debug, Default, Deserialize)]
pub struct OrdersQuery {
    /// Cursor for pagination.
    pub cursor: Option<String>,
    /// Free-text search query.
    pub query: Option<String>,
    /// Sort column key.
    pub sort: Option<String>,
    /// Sort direction (asc/desc).
    pub dir: Option<String>,
    /// Financial status filter.
    pub financial_status: Option<String>,
    /// Fulfillment status filter.
    pub fulfillment_status: Option<String>,
    /// Return status filter.
    pub return_status: Option<String>,
    /// Order status (open/closed/cancelled).
    pub status: Option<String>,
    /// Risk level filter.
    pub risk_level: Option<String>,
    /// Delivery method filter (shipping, `local_delivery`, pickup).
    pub delivery_method: Option<String>,
    /// Sales channel filter.
    pub channel: Option<String>,
    /// Tag filter.
    pub tag: Option<String>,
    /// Discount code filter.
    pub discount_code: Option<String>,
    /// Created date from.
    pub created_at_from: Option<String>,
    /// Created date to.
    pub created_at_to: Option<String>,
}

/// Column visibility state for orders table.
// Allow: This struct represents toggleable UI columns - each needs an independent bool.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default)]
pub struct OrderColumnVisibility {
    pub order: bool,
    pub customer: bool,
    pub payment: bool,
    pub fulfillment: bool,
    pub return_status: bool,
    pub items: bool,
    pub total: bool,
    pub delivery: bool,
    pub channel: bool,
    pub tags: bool,
    pub risk: bool,
    pub destination: bool,
}

impl OrderColumnVisibility {
    /// Create from a list of visible column keys.
    #[must_use]
    pub fn from_columns(columns: &[String]) -> Self {
        Self {
            order: columns.contains(&"order".to_string()),
            customer: columns.contains(&"customer".to_string()),
            payment: columns.contains(&"payment".to_string()),
            fulfillment: columns.contains(&"fulfillment".to_string()),
            return_status: columns.contains(&"return".to_string()),
            items: columns.contains(&"items".to_string()),
            total: columns.contains(&"total".to_string()),
            delivery: columns.contains(&"delivery".to_string()),
            channel: columns.contains(&"channel".to_string()),
            tags: columns.contains(&"tags".to_string()),
            risk: columns.contains(&"risk".to_string()),
            destination: columns.contains(&"destination".to_string()),
        }
    }

    /// Check if a column is visible by key.
    #[must_use]
    pub fn is_visible(&self, key: &str) -> bool {
        match key {
            "order" => self.order,
            "customer" => self.customer,
            "payment" => self.payment,
            "fulfillment" => self.fulfillment,
            "return" => self.return_status,
            "items" => self.items,
            "total" => self.total,
            "delivery" => self.delivery,
            "channel" => self.channel,
            "tags" => self.tags,
            "risk" => self.risk,
            "destination" => self.destination,
            _ => true,
        }
    }
}

/// Order view for data table list display.
#[derive(Debug, Clone)]
pub struct OrderTableView {
    /// Short numeric ID for URLs.
    pub short_id: String,
    /// Full Shopify GID.
    pub id: String,
    /// Order name (e.g., "#1001").
    pub name: String,
    /// Creation date formatted.
    pub created_at: String,
    /// Customer display name.
    pub customer_name: String,
    /// Customer email.
    pub customer_email: Option<String>,
    /// Financial status display text.
    pub payment_status: String,
    /// Financial status badge class.
    pub payment_status_class: String,
    /// Fulfillment status display text.
    pub fulfillment_status: String,
    /// Fulfillment status badge class.
    pub fulfillment_status_class: String,
    /// Return status display text.
    pub return_status: Option<String>,
    /// Return status badge class.
    pub return_status_class: String,
    /// Line item count.
    pub item_count: i64,
    /// Total price formatted.
    pub total: String,
    /// Delivery method (Shipping, Pickup, etc.).
    pub delivery_method: Option<String>,
    /// Sales channel name.
    pub channel: Option<String>,
    /// Order tags.
    pub tags: Vec<String>,
    /// Risk level display.
    pub risk_level: Option<String>,
    /// Risk level badge class.
    pub risk_class: String,
    /// Destination (city, country).
    pub destination: Option<String>,
    /// Whether order is test mode.
    pub is_test: bool,
    /// Whether order is cancelled.
    pub is_cancelled: bool,
    /// Whether order is archived/closed.
    pub is_archived: bool,
}

/// Legacy order view for templates (kept for backward compatibility).
#[derive(Debug, Clone)]
pub struct OrderView {
    pub id: String,
    pub name: String,
    pub customer_name: String,
    pub customer_email: Option<String>,
    pub total: String,
    pub status: String,
    pub status_class: String,
    pub created_at: String,
    pub item_count: i64,
}

// =============================================================================
// Type Conversions and Helpers
// =============================================================================

/// Extract numeric ID from Shopify GID.
fn extract_numeric_id(gid: &str) -> String {
    gid.split('/').next_back().unwrap_or(gid).to_string()
}

/// Format a Shopify Money type as a price string.
fn format_price(money: &Money) -> String {
    money.amount.parse::<f64>().map_or_else(
        |_| format!("${}", money.amount),
        |amount| format!("${amount:.2}"),
    )
}

/// Format financial status with semantic badge class.
fn format_financial_status(status: Option<&FinancialStatus>) -> (String, String) {
    match status {
        Some(FinancialStatus::Paid) => ("Paid".to_string(), "badge badge-success".to_string()),
        Some(FinancialStatus::Authorized) => {
            ("Authorized".to_string(), "badge badge-info".to_string())
        }
        Some(FinancialStatus::PartiallyPaid) => (
            "Partially Paid".to_string(),
            "badge badge-warning".to_string(),
        ),
        Some(FinancialStatus::PartiallyRefunded) => (
            "Partially Refunded".to_string(),
            "badge badge-neutral".to_string(),
        ),
        Some(FinancialStatus::Refunded) => {
            ("Refunded".to_string(), "badge badge-neutral".to_string())
        }
        Some(FinancialStatus::Voided) => {
            ("Voided".to_string(), "badge badge-destructive".to_string())
        }
        Some(FinancialStatus::Pending | FinancialStatus::Expired) | None => {
            ("Pending".to_string(), "badge badge-warning".to_string())
        }
    }
}

/// Format fulfillment status with semantic badge class.
fn format_fulfillment_status(status: Option<&FulfillmentStatus>) -> (String, String) {
    match status {
        Some(FulfillmentStatus::Fulfilled) => {
            ("Fulfilled".to_string(), "badge badge-success".to_string())
        }
        Some(FulfillmentStatus::PartiallyFulfilled) => {
            ("Partial".to_string(), "badge badge-warning".to_string())
        }
        Some(FulfillmentStatus::OnHold) => {
            ("On Hold".to_string(), "badge badge-destructive".to_string())
        }
        Some(FulfillmentStatus::InProgress) => {
            ("In Progress".to_string(), "badge badge-info".to_string())
        }
        Some(FulfillmentStatus::Scheduled) => {
            ("Scheduled".to_string(), "badge badge-info".to_string())
        }
        Some(FulfillmentStatus::Unfulfilled) | None => {
            ("Unfulfilled".to_string(), "badge badge-warning".to_string())
        }
        _ => ("Pending".to_string(), "badge badge-neutral".to_string()),
    }
}

/// Format return status with semantic badge class.
fn format_return_status(status: Option<&OrderReturnStatus>) -> (Option<String>, String) {
    match status {
        Some(OrderReturnStatus::ReturnRequested) => (
            Some("Return Requested".to_string()),
            "badge badge-return".to_string(),
        ),
        Some(OrderReturnStatus::InProgress) => (
            Some("In Progress".to_string()),
            "badge badge-return".to_string(),
        ),
        Some(OrderReturnStatus::Returned) => (
            Some("Returned".to_string()),
            "badge badge-neutral".to_string(),
        ),
        Some(OrderReturnStatus::NoReturn) | None => (None, String::new()),
    }
}

/// Format risk level from order risks.
fn format_risk_level(risks: &[crate::shopify::types::OrderRisk]) -> (Option<String>, String) {
    let highest_risk = risks.iter().map(|r| &r.level).max_by_key(|l| match l {
        OrderRiskLevel::High => 3,
        OrderRiskLevel::Medium => 2,
        OrderRiskLevel::Low => 1,
    });

    match highest_risk {
        Some(OrderRiskLevel::High) => (
            Some("High".to_string()),
            "badge badge-destructive".to_string(),
        ),
        Some(OrderRiskLevel::Medium) => (
            Some("Medium".to_string()),
            "badge badge-warning".to_string(),
        ),
        Some(OrderRiskLevel::Low) => (Some("Low".to_string()), "badge badge-success".to_string()),
        None => (None, String::new()),
    }
}

/// Format destination from shipping address.
fn format_destination(addr: &Address) -> String {
    let city = addr.city.as_deref().unwrap_or("");
    let country = addr.country_code.as_deref().unwrap_or("");
    if city.is_empty() {
        country.to_string()
    } else if country.is_empty() {
        city.to_string()
    } else {
        format!("{city}, {country}")
    }
}

/// Get customer name from order list item using addresses.
fn get_customer_name_from_order(order: &OrderListItem) -> String {
    if let Some(addr) = &order.shipping_address {
        let first = addr.first_name.as_deref().unwrap_or("");
        let last = addr.last_name.as_deref().unwrap_or("");
        let name = format!("{first} {last}").trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    if let Some(addr) = &order.billing_address {
        let first = addr.first_name.as_deref().unwrap_or("");
        let last = addr.last_name.as_deref().unwrap_or("");
        let name = format!("{first} {last}").trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    order.email.clone().unwrap_or_else(|| "Guest".to_string())
}

/// Get customer name from an order (for detail view).
fn get_customer_name(order: &Order) -> String {
    if let Some(addr) = &order.shipping_address {
        let first = addr.first_name.as_deref().unwrap_or("");
        let last = addr.last_name.as_deref().unwrap_or("");
        let name = format!("{first} {last}").trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    if let Some(addr) = &order.billing_address {
        let first = addr.first_name.as_deref().unwrap_or("");
        let last = addr.last_name.as_deref().unwrap_or("");
        let name = format!("{first} {last}").trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    order.email.clone().unwrap_or_else(|| "Guest".to_string())
}

/// Build Shopify query string from filter parameters.
fn build_shopify_query(query: &OrdersQuery) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    // Add free-text search
    if let Some(q) = &query.query
        && !q.is_empty()
    {
        parts.push(q.clone());
    }

    // Financial status filter
    if let Some(status) = &query.financial_status {
        for s in status.split(',') {
            parts.push(format!("financial_status:{s}"));
        }
    }

    // Fulfillment status filter
    if let Some(status) = &query.fulfillment_status {
        for s in status.split(',') {
            parts.push(format!("fulfillment_status:{s}"));
        }
    }

    // Return status filter
    if let Some(status) = &query.return_status {
        for s in status.split(',') {
            parts.push(format!("return_status:{s}"));
        }
    }

    // Order status (open/closed/cancelled)
    if let Some(status) = &query.status {
        parts.push(format!("status:{status}"));
    }

    // Risk level filter
    if let Some(risk) = &query.risk_level {
        parts.push(format!("risk_level:{risk}"));
    }

    // Delivery method filter (maps to shipping_method_category in Shopify)
    if let Some(method) = &query.delivery_method {
        for m in method.split(',') {
            parts.push(format!("shipping_method_category:{m}"));
        }
    }

    // Sales channel filter
    if let Some(channel) = &query.channel
        && !channel.is_empty()
    {
        parts.push(format!("sales_channel:{channel}"));
    }

    // Tag filter
    if let Some(tag) = &query.tag
        && !tag.is_empty()
    {
        parts.push(format!("tag:{tag}"));
    }

    // Discount code filter
    if let Some(code) = &query.discount_code
        && !code.is_empty()
    {
        parts.push(format!("discount_code:{code}"));
    }

    // Date range
    if let Some(from) = &query.created_at_from
        && !from.is_empty()
    {
        parts.push(format!("created_at:>={from}"));
    }
    if let Some(to) = &query.created_at_to
        && !to.is_empty()
    {
        parts.push(format!("created_at:<={to}"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

/// Build URL parameters for preserving filters across pagination.
fn build_preserve_params(query: &OrdersQuery) -> String {
    let mut params = Vec::new();

    if let Some(q) = &query.query
        && !q.is_empty()
    {
        params.push(format!("query={}", urlencoding::encode(q)));
    }
    if let Some(s) = &query.sort {
        params.push(format!("sort={s}"));
    }
    if let Some(d) = &query.dir {
        params.push(format!("dir={d}"));
    }
    if let Some(fs) = &query.financial_status {
        params.push(format!("financial_status={fs}"));
    }
    if let Some(fs) = &query.fulfillment_status {
        params.push(format!("fulfillment_status={fs}"));
    }
    if let Some(rs) = &query.return_status {
        params.push(format!("return_status={rs}"));
    }
    if let Some(s) = &query.status {
        params.push(format!("status={s}"));
    }
    if let Some(rl) = &query.risk_level {
        params.push(format!("risk_level={rl}"));
    }
    if let Some(dm) = &query.delivery_method {
        params.push(format!("delivery_method={dm}"));
    }
    if let Some(ch) = &query.channel
        && !ch.is_empty()
    {
        params.push(format!("channel={}", urlencoding::encode(ch)));
    }
    if let Some(t) = &query.tag
        && !t.is_empty()
    {
        params.push(format!("tag={}", urlencoding::encode(t)));
    }
    if let Some(dc) = &query.discount_code
        && !dc.is_empty()
    {
        params.push(format!("discount_code={}", urlencoding::encode(dc)));
    }
    if let Some(from) = &query.created_at_from
        && !from.is_empty()
    {
        params.push(format!("created_at_from={from}"));
    }
    if let Some(to) = &query.created_at_to
        && !to.is_empty()
    {
        params.push(format!("created_at_to={to}"));
    }

    if params.is_empty() {
        String::new()
    } else {
        format!("&{}", params.join("&"))
    }
}

impl From<&OrderListItem> for OrderTableView {
    fn from(order: &OrderListItem) -> Self {
        let short_id = extract_numeric_id(&order.id);
        let (payment_status, payment_status_class) =
            format_financial_status(order.financial_status.as_ref());
        let (fulfillment_status, fulfillment_status_class) =
            format_fulfillment_status(order.fulfillment_status.as_ref());
        let (return_status, return_status_class) =
            format_return_status(order.return_status.as_ref());
        let (risk_level, risk_class) = format_risk_level(&order.risks);
        let destination = order.shipping_address.as_ref().map(format_destination);
        let delivery_method = order
            .shipping_line
            .as_ref()
            .and_then(|sl| sl.delivery_category.as_ref())
            .map(|dc| format!("{dc}"));

        Self {
            short_id,
            id: order.id.clone(),
            name: order.name.clone(),
            created_at: order.created_at.clone(),
            customer_name: order
                .customer_name
                .clone()
                .unwrap_or_else(|| get_customer_name_from_order(order)),
            customer_email: order.email.clone(),
            payment_status,
            payment_status_class,
            fulfillment_status,
            fulfillment_status_class,
            return_status,
            return_status_class,
            item_count: order.total_items_quantity,
            total: format_price(&order.total_price),
            delivery_method,
            channel: order
                .channel_info
                .as_ref()
                .and_then(|ci| ci.channel_name.clone()),
            tags: order.tags.clone(),
            risk_level,
            risk_class,
            destination,
            is_test: order.test,
            is_cancelled: order.cancelled,
            is_archived: order.closed,
        }
    }
}

// =============================================================================
// Legacy Type Conversions (for detail view)
// =============================================================================

/// Map fulfillment status to display string and CSS class.
fn fulfillment_status_display(order: &Order) -> (String, String) {
    match order.fulfillment_status {
        Some(FulfillmentStatus::Fulfilled) => (
            "Fulfilled".to_string(),
            "bg-green-100 text-green-700".to_string(),
        ),
        Some(FulfillmentStatus::PartiallyFulfilled) => (
            "Partially Fulfilled".to_string(),
            "bg-blue-100 text-blue-700".to_string(),
        ),
        Some(FulfillmentStatus::Unfulfilled) | None => (
            "Unfulfilled".to_string(),
            "bg-yellow-100 text-yellow-700".to_string(),
        ),
        Some(FulfillmentStatus::OnHold) => {
            ("On Hold".to_string(), "bg-red-100 text-red-700".to_string())
        }
        Some(FulfillmentStatus::InProgress) => (
            "In Progress".to_string(),
            "bg-blue-100 text-blue-700".to_string(),
        ),
        _ => (
            "Pending".to_string(),
            "bg-gray-100 text-gray-700".to_string(),
        ),
    }
}

impl From<&Order> for OrderView {
    fn from(order: &Order) -> Self {
        let (status, status_class) = fulfillment_status_display(order);
        let item_count: i64 = order.line_items.iter().map(|li| li.quantity).sum();

        Self {
            id: order.id.clone(),
            name: order.name.clone(),
            customer_name: get_customer_name(order),
            customer_email: order.email.clone(),
            total: format_price(&order.total_price),
            status,
            status_class,
            created_at: order.created_at.clone(),
            item_count,
        }
    }
}

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

// =============================================================================
// Order Detail Views
// =============================================================================

/// Line item view for templates.
#[derive(Debug, Clone)]
pub struct LineItemView {
    pub title: String,
    pub variant_title: Option<String>,
    pub sku: Option<String>,
    pub quantity: i64,
    pub unit_price: String,
    pub total_price: String,
}

// Precision loss from i64 -> f64 is acceptable here since order quantities
// are realistically bounded (never approaching 2^53) and this is for display only.
#[allow(clippy::cast_precision_loss)]
impl From<&OrderLineItem> for LineItemView {
    fn from(item: &OrderLineItem) -> Self {
        let unit_price = format_price(&item.discounted_unit_price);
        let total = item
            .discounted_unit_price
            .amount
            .parse::<f64>()
            .unwrap_or(0.0)
            * item.quantity as f64;
        Self {
            title: item.title.clone(),
            variant_title: item.variant_title.clone(),
            sku: item.sku.clone(),
            quantity: item.quantity,
            unit_price,
            total_price: format!("${total:.2}"),
        }
    }
}

/// Address view for templates.
#[derive(Debug, Clone)]
pub struct AddressView {
    pub name: String,
    pub company: Option<String>,
    pub address1: String,
    pub address2: Option<String>,
    pub city: String,
    pub province_code: String,
    pub zip: String,
    pub country: String,
    pub phone: Option<String>,
    // Legacy fields for backward compatibility
    pub line1: String,
    pub line2: Option<String>,
    pub city_state_zip: String,
}

impl From<&Address> for AddressView {
    fn from(addr: &Address) -> Self {
        let first = addr.first_name.as_deref().unwrap_or("");
        let last = addr.last_name.as_deref().unwrap_or("");
        let name = format!("{first} {last}").trim().to_string();

        let address1 = addr.address1.clone().unwrap_or_default();
        let address2 = addr.address2.clone().filter(|s| !s.is_empty());

        let city = addr.city.as_deref().unwrap_or("").to_string();
        let province_code = addr.province_code.as_deref().unwrap_or("").to_string();
        let zip = addr.zip.as_deref().unwrap_or("").to_string();
        let city_state_zip = format!("{city}, {province_code} {zip}").trim().to_string();

        let country = addr.country_code.clone().unwrap_or_default();

        Self {
            name: if name.is_empty() {
                "N/A".to_string()
            } else {
                name
            },
            company: addr.company.clone(),
            address1: address1.clone(),
            address2: address2.clone(),
            city,
            province_code,
            zip,
            country,
            phone: addr.phone.clone(),
            // Legacy fields
            line1: address1,
            line2: address2,
            city_state_zip,
        }
    }
}

/// Fulfilled line item view for templates.
#[derive(Debug, Clone)]
pub struct FulfilledLineItemView {
    pub title: String,
    pub quantity: i64,
}

/// Fulfillment view for templates.
#[derive(Debug, Clone)]
pub struct FulfillmentView {
    pub id: String,
    pub status: String,
    pub tracking_number: Option<String>,
    pub tracking_url: Option<String>,
    pub carrier: Option<String>,
    pub location_name: Option<String>,
    pub created_at: String,
    pub line_items: Vec<FulfilledLineItemView>,
}

impl From<&Fulfillment> for FulfillmentView {
    fn from(f: &Fulfillment) -> Self {
        let tracking = f.tracking_info.first();
        Self {
            id: f.id.clone(),
            status: f.status.clone(),
            tracking_number: tracking.and_then(|t| t.number.clone()),
            tracking_url: tracking.and_then(|t| t.url.clone()),
            carrier: tracking.and_then(|t| t.company.clone()),
            location_name: None,
            created_at: f.created_at.clone(),
            line_items: vec![],
        }
    }
}

/// Transaction view for templates.
#[derive(Debug, Clone)]
pub struct TransactionView {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub amount: String,
    pub gateway: Option<String>,
    pub created_at: String,
}

/// Fulfillment order line item view for templates.
#[derive(Debug, Clone)]
pub struct FulfillmentOrderLineItemView {
    pub id: String,
    pub title: String,
    pub variant_title: Option<String>,
    pub sku: Option<String>,
    pub image: Option<String>,
    pub total_quantity: i64,
    pub remaining_quantity: i64,
}

/// Fulfillment order view for templates.
#[derive(Debug, Clone)]
pub struct FulfillmentOrderView {
    pub id: String,
    pub status: String,
    pub location_name: Option<String>,
    pub line_items: Vec<FulfillmentOrderLineItemView>,
}

/// Risk view for templates.
#[derive(Debug, Clone)]
pub struct RiskView {
    pub level: String,
    pub message: Option<String>,
    pub provider: Option<String>,
}

/// Timeline event view for templates.
#[derive(Debug, Clone)]
pub struct TimelineEventView {
    pub event_type: String,
    pub message: String,
    pub created_at: String,
    pub staff_name: Option<String>,
}

/// Order detail view for templates with full enhanced data.
// View structs for templates need multiple bool flags for display logic.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct OrderDetailView {
    // Basic order info
    pub id: String,
    pub short_id: String,
    pub name: String,
    pub created_at: String,

    // Status fields
    pub financial_status: String,
    pub financial_status_class: String,
    pub fulfillment_status: String,
    pub fulfillment_status_class: String,
    pub return_status: Option<String>,
    pub return_status_class: String,

    // Flags
    pub is_paid: bool,
    pub is_test: bool,
    pub is_cancelled: bool,
    pub is_archived: bool,
    pub has_capturable: bool,
    pub capturable_amount: String,

    // Pricing
    pub subtotal: String,
    pub shipping: String,
    pub tax: String,
    pub discount: String,
    pub total: String,
    pub total_paid: String,
    pub total_refunded: String,
    pub net_payment: String,
    pub total_outstanding: String,

    // Customer
    pub customer_id: Option<String>,
    pub customer_name: String,
    pub customer_email: Option<String>,
    pub customer_phone: Option<String>,
    pub orders_count: i64,
    pub total_spent: String,
    pub customer_note: Option<String>,
    pub email_marketing: bool,

    // Note and tags
    pub note: Option<String>,
    pub tags: Vec<String>,

    // Items and fulfillments
    pub line_items: Vec<LineItemView>,
    pub fulfillment_orders: Vec<FulfillmentOrderView>,
    pub fulfillments: Vec<FulfillmentView>,

    // Transactions, risks, timeline
    pub transactions: Vec<TransactionView>,
    pub risks: Vec<RiskView>,
    pub events: Vec<TimelineEventView>,

    // Addresses
    pub shipping_address: Option<AddressView>,
    pub billing_address: Option<AddressView>,
}

/// Map financial status to display string and CSS class.
fn financial_status_display(order: &Order) -> (String, String, bool) {
    match order.financial_status {
        Some(FinancialStatus::Paid) => (
            "Paid".to_string(),
            "bg-green-100 text-green-700".to_string(),
            true,
        ),
        Some(FinancialStatus::Authorized) => (
            "Authorized".to_string(),
            "bg-blue-100 text-blue-700".to_string(),
            false,
        ),
        Some(FinancialStatus::Pending) => (
            "Pending".to_string(),
            "bg-yellow-100 text-yellow-700".to_string(),
            false,
        ),
        Some(FinancialStatus::PartiallyPaid) => (
            "Partially Paid".to_string(),
            "bg-yellow-100 text-yellow-700".to_string(),
            false,
        ),
        Some(FinancialStatus::Refunded) => (
            "Refunded".to_string(),
            "bg-gray-100 text-gray-700".to_string(),
            true,
        ),
        Some(FinancialStatus::PartiallyRefunded) => (
            "Partially Refunded".to_string(),
            "bg-gray-100 text-gray-700".to_string(),
            true,
        ),
        Some(FinancialStatus::Voided) => (
            "Voided".to_string(),
            "bg-red-100 text-red-700".to_string(),
            false,
        ),
        _ => (
            "Unknown".to_string(),
            "bg-gray-100 text-gray-700".to_string(),
            false,
        ),
    }
}

impl From<&Order> for OrderDetailView {
    fn from(order: &Order) -> Self {
        let short_id = extract_numeric_id(&order.id);
        let (fulfillment_status, fulfillment_status_class) = fulfillment_status_display(order);
        let (financial_status, financial_status_class, is_paid) = financial_status_display(order);
        let total_str = format_price(&order.total_price);

        Self {
            id: order.id.clone(),
            short_id,
            name: order.name.clone(),
            created_at: order.created_at.clone(),

            financial_status,
            financial_status_class,
            fulfillment_status,
            fulfillment_status_class,
            return_status: None,
            return_status_class: String::new(),

            is_paid,
            is_test: order.test,
            is_cancelled: false,
            is_archived: false,
            has_capturable: false,
            capturable_amount: "$0.00".to_string(),

            subtotal: format_price(&order.subtotal_price),
            shipping: format_price(&order.total_shipping_price),
            tax: format_price(&order.total_tax),
            discount: format_price(&order.total_discounts),
            total: total_str.clone(),
            total_paid: if is_paid {
                total_str
            } else {
                "$0.00".to_string()
            },
            total_refunded: "$0.00".to_string(),
            net_payment: format_price(&order.total_price),
            total_outstanding: if is_paid {
                "$0.00".to_string()
            } else {
                format_price(&order.total_price)
            },

            customer_id: order.customer_id.as_ref().map(|id| extract_numeric_id(id)),
            customer_name: get_customer_name(order),
            customer_email: order.email.clone(),
            customer_phone: order.phone.clone(),
            orders_count: 0,
            total_spent: "$0.00".to_string(),
            customer_note: None,
            email_marketing: false,

            note: order.note.clone(),
            tags: vec![],

            line_items: order.line_items.iter().map(LineItemView::from).collect(),
            fulfillment_orders: vec![],
            fulfillments: order
                .fulfillments
                .iter()
                .map(FulfillmentView::from)
                .collect(),

            transactions: vec![],
            risks: vec![],
            events: vec![],

            shipping_address: order.shipping_address.as_ref().map(AddressView::from),
            billing_address: order.billing_address.as_ref().map(AddressView::from),
        }
    }
}

/// Order detail page template.
#[derive(Template)]
#[template(path = "orders/show.html")]
pub struct OrderShowTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub order: OrderDetailView,
    pub error: Option<String>,
}

/// Note update form input.
#[derive(Debug, Deserialize)]
pub struct NoteFormInput {
    pub note: Option<String>,
}

/// Cancel form input.
#[derive(Debug, Deserialize)]
pub struct CancelFormInput {
    pub reason: Option<String>,
    pub notify_customer: Option<String>,
    pub refund: Option<String>,
    pub restock: Option<String>,
}

/// Order detail page handler.
#[instrument(skip(admin, state))]
pub async fn show(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state.shopify().get_order(&order_id).await {
        Ok(Some(order)) => {
            let template = OrderShowTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/orders".to_string(),
                order: OrderDetailView::from(&order),
                error: None,
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Order not found").into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch order: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch order").into_response()
        }
    }
}

/// Update order note handler.
#[instrument(skip(admin, state))]
pub async fn update_note(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<NoteFormInput>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state
        .shopify()
        .update_order_note(&order_id, input.note.as_deref())
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %order_id, "Order note updated");
            // Redirect back to order detail
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to update order note");
            // Fetch order again and show error
            match state.shopify().get_order(&order_id).await {
                Ok(Some(order)) => {
                    let template = OrderShowTemplate {
                        admin_user: AdminUserView::from(&admin),
                        current_path: "/orders".to_string(),
                        order: OrderDetailView::from(&order),
                        error: Some(format!("Failed to update note: {e}")),
                    };
                    Html(template.render().unwrap_or_else(|e| {
                        tracing::error!("Template render error: {}", e);
                        "Internal Server Error".to_string()
                    }))
                    .into_response()
                }
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to update order").into_response(),
            }
        }
    }
}

/// Mark order as paid handler.
#[instrument(skip(_admin, state))]
pub async fn mark_paid(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state.shopify().mark_order_as_paid(&order_id).await {
        Ok(()) => {
            tracing::info!(order_id = %order_id, "Order marked as paid");
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to mark order as paid");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to mark as paid: {e}"),
            )
                .into_response()
        }
    }
}

/// Cancel order handler.
#[instrument(skip(_admin, state))]
pub async fn cancel(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<CancelFormInput>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    let notify = input.notify_customer.as_deref() == Some("on");
    let refund = input.refund.as_deref() == Some("on");
    let restock = input.restock.as_deref() == Some("on");

    match state
        .shopify()
        .cancel_order(&order_id, input.reason.as_deref(), notify, refund, restock)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %order_id, reason = ?input.reason, "Order cancelled");
            Redirect::to("/orders").into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to cancel order");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to cancel order: {e}"),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Bulk Action Handlers
// =============================================================================

/// Input for bulk tag operations.
#[derive(Debug, Deserialize)]
pub struct BulkTagsInput {
    /// Comma-separated list of order IDs.
    pub order_ids: String,
    /// Tags to add or remove (comma-separated).
    pub tags: String,
}

/// Input for bulk archive/cancel operations.
#[derive(Debug, Deserialize)]
pub struct BulkOrdersInput {
    /// Comma-separated list of order IDs.
    pub order_ids: String,
}

/// Bulk add tags to orders.
#[instrument(skip(_admin, state))]
pub async fn bulk_add_tags(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<BulkTagsInput>,
) -> impl IntoResponse {
    let order_ids: Vec<&str> = input.order_ids.split(',').map(str::trim).collect();
    let tags: Vec<String> = input
        .tags
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();

    if order_ids.is_empty() || tags.is_empty() {
        return (StatusCode::BAD_REQUEST, "No orders or tags specified").into_response();
    }

    let mut success_count = 0;
    let mut error_messages = Vec::new();

    for id in &order_ids {
        let order_id = if id.starts_with("gid://") {
            (*id).to_string()
        } else {
            format!("gid://shopify/Order/{id}")
        };

        match state.shopify().add_tags_to_order(&order_id, &tags).await {
            Ok(_) => success_count += 1,
            Err(e) => {
                error_messages.push(format!("{id}: {e}"));
            }
        }
    }

    if error_messages.is_empty() {
        tracing::info!(count = success_count, "Bulk add tags completed");
        Redirect::to("/orders").into_response()
    } else {
        tracing::warn!(
            success = success_count,
            errors = ?error_messages,
            "Bulk add tags completed with errors"
        );
        (
            StatusCode::MULTI_STATUS,
            format!(
                "Added tags to {success_count} orders. Errors: {}",
                error_messages.join("; ")
            ),
        )
            .into_response()
    }
}

/// Bulk remove tags from orders.
#[instrument(skip(_admin, state))]
pub async fn bulk_remove_tags(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<BulkTagsInput>,
) -> impl IntoResponse {
    let order_ids: Vec<&str> = input.order_ids.split(',').map(str::trim).collect();
    let tags: Vec<String> = input
        .tags
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();

    if order_ids.is_empty() || tags.is_empty() {
        return (StatusCode::BAD_REQUEST, "No orders or tags specified").into_response();
    }

    let mut success_count = 0;
    let mut error_messages = Vec::new();

    for id in &order_ids {
        let order_id = if id.starts_with("gid://") {
            (*id).to_string()
        } else {
            format!("gid://shopify/Order/{id}")
        };

        match state
            .shopify()
            .remove_tags_from_order(&order_id, &tags)
            .await
        {
            Ok(_) => success_count += 1,
            Err(e) => {
                error_messages.push(format!("{id}: {e}"));
            }
        }
    }

    if error_messages.is_empty() {
        tracing::info!(count = success_count, "Bulk remove tags completed");
        Redirect::to("/orders").into_response()
    } else {
        tracing::warn!(
            success = success_count,
            errors = ?error_messages,
            "Bulk remove tags completed with errors"
        );
        (
            StatusCode::MULTI_STATUS,
            format!(
                "Removed tags from {success_count} orders. Errors: {}",
                error_messages.join("; ")
            ),
        )
            .into_response()
    }
}

/// Bulk archive orders.
#[instrument(skip(_admin, state))]
pub async fn bulk_archive(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<BulkOrdersInput>,
) -> impl IntoResponse {
    let order_ids: Vec<&str> = input.order_ids.split(',').map(str::trim).collect();

    if order_ids.is_empty() {
        return (StatusCode::BAD_REQUEST, "No orders specified").into_response();
    }

    let mut success_count = 0;
    let mut error_messages = Vec::new();

    for id in &order_ids {
        let order_id = if id.starts_with("gid://") {
            (*id).to_string()
        } else {
            format!("gid://shopify/Order/{id}")
        };

        match state.shopify().archive_order(&order_id).await {
            Ok(()) => success_count += 1,
            Err(e) => {
                error_messages.push(format!("{id}: {e}"));
            }
        }
    }

    if error_messages.is_empty() {
        tracing::info!(count = success_count, "Bulk archive completed");
        Redirect::to("/orders").into_response()
    } else {
        tracing::warn!(
            success = success_count,
            errors = ?error_messages,
            "Bulk archive completed with errors"
        );
        (
            StatusCode::MULTI_STATUS,
            format!(
                "Archived {success_count} orders. Errors: {}",
                error_messages.join("; ")
            ),
        )
            .into_response()
    }
}

/// Bulk cancel orders.
#[instrument(skip(_admin, state))]
pub async fn bulk_cancel(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<BulkOrdersInput>,
) -> impl IntoResponse {
    let order_ids: Vec<&str> = input.order_ids.split(',').map(str::trim).collect();

    if order_ids.is_empty() {
        return (StatusCode::BAD_REQUEST, "No orders specified").into_response();
    }

    let mut success_count = 0;
    let mut error_messages = Vec::new();

    for id in &order_ids {
        let order_id = if id.starts_with("gid://") {
            (*id).to_string()
        } else {
            format!("gid://shopify/Order/{id}")
        };

        // Cancel with default settings: no notification, no refund, restock items
        match state
            .shopify()
            .cancel_order(&order_id, Some("OTHER"), false, false, true)
            .await
        {
            Ok(()) => success_count += 1,
            Err(e) => {
                error_messages.push(format!("{id}: {e}"));
            }
        }
    }

    if error_messages.is_empty() {
        tracing::info!(count = success_count, "Bulk cancel completed");
        Redirect::to("/orders").into_response()
    } else {
        tracing::warn!(
            success = success_count,
            errors = ?error_messages,
            "Bulk cancel completed with errors"
        );
        (
            StatusCode::MULTI_STATUS,
            format!(
                "Cancelled {success_count} orders. Errors: {}",
                error_messages.join("; ")
            ),
        )
            .into_response()
    }
}

// =============================================================================
// Single Order Action Handlers
// =============================================================================

/// Input for adding/removing a single tag.
#[derive(Debug, Deserialize)]
pub struct TagInput {
    /// Tag to add or remove.
    pub tag: String,
    /// Action: "add" or "remove".
    pub action: String,
}

/// Add or remove a tag from an order (HTMX).
#[instrument(skip(_admin, state))]
pub async fn update_tags(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<TagInput>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    let result = if input.action == "remove" {
        state
            .shopify()
            .remove_tags_from_order(&order_id, std::slice::from_ref(&input.tag))
            .await
    } else {
        state
            .shopify()
            .add_tags_to_order(&order_id, std::slice::from_ref(&input.tag))
            .await
    };

    match result {
        Ok(tags) => {
            tracing::info!(order_id = %order_id, action = %input.action, tag = %input.tag, "Tag updated");
            // Return updated tags list as HTML for HTMX swap
            let tags_html: String = tags
                .iter()
                .map(|t| format!(r#"<span class="badge badge-secondary">{t}</span>"#))
                .collect::<Vec<_>>()
                .join(" ");
            Html(tags_html).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to update tag");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to update tag: {e}"),
            )
                .into_response()
        }
    }
}

/// Input for creating a fulfillment.
#[derive(Debug, Deserialize)]
pub struct FulfillInput {
    /// Fulfillment order ID to fulfill.
    pub fulfillment_order_id: String,
    /// Optional tracking company.
    pub tracking_company: Option<String>,
    /// Optional tracking number.
    pub tracking_number: Option<String>,
    /// Optional tracking URL.
    pub tracking_url: Option<String>,
}

/// Create a fulfillment for an order.
#[instrument(skip(_admin, state))]
pub async fn fulfill(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<FulfillInput>,
) -> impl IntoResponse {
    let fulfillment_order_id = if input.fulfillment_order_id.starts_with("gid://") {
        input.fulfillment_order_id.clone()
    } else {
        format!(
            "gid://shopify/FulfillmentOrder/{}",
            input.fulfillment_order_id
        )
    };

    match state
        .shopify()
        .create_fulfillment(
            &fulfillment_order_id,
            input.tracking_company.as_deref(),
            input.tracking_number.as_deref(),
            input.tracking_url.as_deref(),
        )
        .await
    {
        Ok(fulfillment_id) => {
            tracing::info!(order_id = %id, fulfillment_id = %fulfillment_id, "Fulfillment created");
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %id, error = %e, "Failed to create fulfillment");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to create fulfillment: {e}"),
            )
                .into_response()
        }
    }
}

/// Input for holding a fulfillment order.
#[derive(Debug, Deserialize)]
pub struct HoldInput {
    /// Reason for the hold.
    pub reason: String,
    /// Additional notes.
    pub reason_notes: Option<String>,
}

/// Hold a fulfillment order.
#[instrument(skip(_admin, state))]
pub async fn hold_fulfillment(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path((order_id, fo_id)): Path<(String, String)>,
    Form(input): Form<HoldInput>,
) -> impl IntoResponse {
    use crate::shopify::types::{FulfillmentHoldInput, FulfillmentHoldReason};

    let fulfillment_order_id = if fo_id.starts_with("gid://") {
        fo_id.clone()
    } else {
        format!("gid://shopify/FulfillmentOrder/{fo_id}")
    };

    let reason = match input.reason.to_uppercase().as_str() {
        "AWAITING_PAYMENT" => FulfillmentHoldReason::AwaitingPayment,
        "HIGH_RISK_OF_FRAUD" => FulfillmentHoldReason::HighRiskOfFraud,
        "INCORRECT_ADDRESS" => FulfillmentHoldReason::IncorrectAddress,
        "INVENTORY_OUT_OF_STOCK" => FulfillmentHoldReason::InventoryOutOfStock,
        "UNKNOWN_DELIVERY_DATE" => FulfillmentHoldReason::UnknownDeliveryDate,
        "AWAITING_RETURN_ITEMS" => FulfillmentHoldReason::AwaitingReturnItems,
        _ => FulfillmentHoldReason::Other,
    };

    let hold_input = FulfillmentHoldInput {
        reason,
        reason_notes: input.reason_notes,
        notify_merchant: false,
    };

    match state
        .shopify()
        .hold_fulfillment_order(&fulfillment_order_id, hold_input)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %order_id, fulfillment_order_id = %fulfillment_order_id, "Fulfillment order held");
            let numeric_id = order_id.split('/').next_back().unwrap_or(&order_id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(fulfillment_order_id = %fulfillment_order_id, error = %e, "Failed to hold fulfillment order");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to hold fulfillment: {e}"),
            )
                .into_response()
        }
    }
}

/// Release a hold on a fulfillment order.
#[instrument(skip(_admin, state))]
pub async fn release_hold(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path((order_id, fo_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let fulfillment_order_id = if fo_id.starts_with("gid://") {
        fo_id.clone()
    } else {
        format!("gid://shopify/FulfillmentOrder/{fo_id}")
    };

    match state
        .shopify()
        .release_fulfillment_order_hold(&fulfillment_order_id)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %order_id, fulfillment_order_id = %fulfillment_order_id, "Fulfillment hold released");
            let numeric_id = order_id.split('/').next_back().unwrap_or(&order_id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(fulfillment_order_id = %fulfillment_order_id, error = %e, "Failed to release hold");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to release hold: {e}"),
            )
                .into_response()
        }
    }
}

/// Calculate suggested refund for an order (HTMX).
///
/// Returns HTML partial with refund calculation details.
#[instrument(skip(_admin, state))]
pub async fn calculate_refund(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state.shopify().get_suggested_refund(&order_id).await {
        Ok(suggested) => {
            use std::fmt::Write;

            // Return HTML partial for HTMX swap
            let mut line_items_html = String::new();
            for item in &suggested.line_items {
                let _ = write!(
                    line_items_html,
                    r#"<div class="flex justify-between py-2 border-b border-muted">
                        <span>{} (qty: {})</span>
                        <span>Refund: {}</span>
                    </div>"#,
                    item.title, item.original_quantity, item.refund_quantity
                );
            }

            let html = format!(
                r#"<div class="space-y-4">
                    <div class="text-sm text-muted-foreground">Suggested refund calculation</div>
                    <div class="space-y-2">{line_items_html}</div>
                    <div class="border-t border-muted pt-4 space-y-2">
                        <div class="flex justify-between">
                            <span>Subtotal</span>
                            <span>{} {}</span>
                        </div>
                        <div class="flex justify-between">
                            <span>Tax</span>
                            <span>{} {}</span>
                        </div>
                        <div class="flex justify-between font-semibold">
                            <span>Total refund</span>
                            <span>{} {}</span>
                        </div>
                    </div>
                </div>"#,
                suggested.subtotal,
                suggested.currency_code,
                suggested.total_tax,
                suggested.currency_code,
                suggested.amount,
                suggested.currency_code
            );
            Html(html).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to calculate refund");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to calculate refund: {e}"),
            )
                .into_response()
        }
    }
}

/// Input for creating a refund.
#[derive(Debug, Deserialize)]
pub struct RefundInput {
    /// Refund note.
    pub note: Option<String>,
    /// Whether to notify customer.
    pub notify: Option<String>,
    /// Comma-separated line item IDs and quantities (format: "id:qty,id:qty").
    pub line_items: Option<String>,
    /// Shipping refund amount.
    pub shipping_amount: Option<String>,
    /// Whether to do full shipping refund.
    pub full_shipping_refund: Option<String>,
}

/// Create a refund for an order.
#[instrument(skip(_admin, state))]
pub async fn refund(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<RefundInput>,
) -> impl IntoResponse {
    use crate::shopify::types::{RefundCreateInput, RefundLineItemInput, RefundRestockType};

    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    // Parse line items from "id:qty,id:qty" format
    let line_items: Vec<RefundLineItemInput> = input
        .line_items
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter_map(|item| {
            let mut parts = item.trim().split(':');
            let id_part = parts.next()?;
            let qty_part = parts.next()?;
            // Ensure no extra parts
            if parts.next().is_some() {
                return None;
            }
            let line_item_id = if id_part.starts_with("gid://") {
                id_part.to_string()
            } else {
                format!("gid://shopify/LineItem/{id_part}")
            };
            let quantity = qty_part.parse().ok()?;
            Some(RefundLineItemInput {
                line_item_id,
                quantity,
                restock_type: RefundRestockType::Return,
                location_id: None,
            })
        })
        .collect();

    let refund_input = RefundCreateInput {
        note: input.note,
        notify: input.notify.as_deref() == Some("on"),
        line_items,
        shipping_amount: input.shipping_amount,
        full_shipping_refund: input.full_shipping_refund.as_deref() == Some("on"),
    };

    match state.shopify().create_refund(&order_id, refund_input).await {
        Ok(refund_id) => {
            tracing::info!(order_id = %order_id, refund_id = %refund_id, "Refund created");
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to create refund");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to create refund: {e}"),
            )
                .into_response()
        }
    }
}

/// Input for creating a return.
#[derive(Debug, Deserialize)]
pub struct ReturnInput {
    /// Comma-separated fulfillment line item IDs and quantities (format: "id:qty,id:qty").
    pub line_items: String,
    /// Return reason note.
    pub reason_note: Option<String>,
}

/// Create a return for an order.
#[instrument(skip(_admin, state))]
pub async fn create_return(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<ReturnInput>,
) -> impl IntoResponse {
    use crate::shopify::types::{ReturnCreateInput, ReturnLineItemCreateInput};

    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    // Parse line items from "id:qty,id:qty" format
    let line_items: Vec<ReturnLineItemCreateInput> = input
        .line_items
        .split(',')
        .filter_map(|item| {
            let mut parts = item.trim().split(':');
            let id_part = parts.next()?;
            let qty_part = parts.next()?;
            // Ensure no extra parts
            if parts.next().is_some() {
                return None;
            }
            let fulfillment_line_item_id = if id_part.starts_with("gid://") {
                id_part.to_string()
            } else {
                format!("gid://shopify/FulfillmentLineItem/{id_part}")
            };
            let quantity = qty_part.parse().ok()?;
            Some(ReturnLineItemCreateInput {
                fulfillment_line_item_id,
                quantity,
                return_reason_note: input.reason_note.clone(),
            })
        })
        .collect();

    if line_items.is_empty() {
        return (StatusCode::BAD_REQUEST, "No valid line items specified").into_response();
    }

    let return_input = ReturnCreateInput {
        line_items,
        requested_at: None,
    };

    match state.shopify().create_return(&order_id, return_input).await {
        Ok(return_id) => {
            tracing::info!(order_id = %order_id, return_id = %return_id, "Return created");
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to create return");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to create return: {e}"),
            )
                .into_response()
        }
    }
}

/// Input for capturing payment.
#[derive(Debug, Deserialize)]
pub struct CaptureInput {
    /// Transaction ID to capture.
    pub transaction_id: String,
    /// Amount to capture.
    pub amount: String,
}

/// Capture payment on an order.
#[instrument(skip(_admin, state))]
pub async fn capture(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<CaptureInput>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    let transaction_id = if input.transaction_id.starts_with("gid://") {
        input.transaction_id.clone()
    } else {
        format!("gid://shopify/OrderTransaction/{}", input.transaction_id)
    };

    match state
        .shopify()
        .capture_order_payment(&order_id, &transaction_id, &input.amount)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %order_id, "Payment captured");
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to capture payment");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to capture payment: {e}"),
            )
                .into_response()
        }
    }
}

/// Archive or unarchive an order.
#[instrument(skip(_admin, state))]
pub async fn archive(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<ArchiveParams>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    let result = if params.unarchive.unwrap_or(false) {
        state.shopify().unarchive_order(&order_id).await
    } else {
        state.shopify().archive_order(&order_id).await
    };

    match result {
        Ok(()) => {
            let action = if params.unarchive.unwrap_or(false) {
                "unarchived"
            } else {
                "archived"
            };
            tracing::info!(order_id = %order_id, action = action, "Order archive status updated");
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to update archive status");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to update archive status: {e}"),
            )
                .into_response()
        }
    }
}

/// Query params for archive action.
#[derive(Debug, Deserialize)]
pub struct ArchiveParams {
    /// If true, unarchive instead of archive.
    pub unarchive: Option<bool>,
}

// =============================================================================
// Print Handlers
// =============================================================================

/// Query params for print action.
#[derive(Debug, Deserialize)]
pub struct PrintQuery {
    /// Type of document: "invoice" or `packing_slip`.
    #[serde(rename = "type")]
    pub doc_type: Option<String>,
}

/// Print line item view (simpler than full line item view).
#[derive(Debug, Clone)]
pub struct PrintLineItemView {
    pub title: String,
    pub variant_title: Option<String>,
    pub sku: Option<String>,
    pub quantity: i64,
    pub price: String,
    pub total: String,
}

// Precision loss from i64 -> f64 is acceptable for display.
#[allow(clippy::cast_precision_loss)]
impl From<&OrderLineItem> for PrintLineItemView {
    fn from(item: &OrderLineItem) -> Self {
        let price = format_price(&item.discounted_unit_price);
        let total = item
            .discounted_unit_price
            .amount
            .parse::<f64>()
            .unwrap_or(0.0)
            * item.quantity as f64;
        Self {
            title: item.title.clone(),
            variant_title: item.variant_title.clone(),
            sku: item.sku.clone(),
            quantity: item.quantity,
            price,
            total: format!("${total:.2}"),
        }
    }
}

/// Order view for print templates.
#[derive(Debug, Clone)]
pub struct PrintOrderView {
    pub name: String,
    pub created_at: String,
    pub financial_status: String,
    pub subtotal: String,
    pub shipping: String,
    pub discount: String,
    pub tax: String,
    pub total: String,
    pub note: Option<String>,
    pub shipping_method: Option<String>,
    pub shipping_address: Option<AddressView>,
    pub billing_address: Option<AddressView>,
}

/// Invoice print template.
#[derive(Template)]
#[template(path = "orders/print_invoice.html")]
pub struct OrderInvoiceTemplate {
    pub order: PrintOrderView,
    pub line_items: Vec<PrintLineItemView>,
    pub printed_at: String,
}

/// Packing slip print template.
#[derive(Template)]
#[template(path = "orders/print_packing_slip.html")]
pub struct OrderPackingSlipTemplate {
    pub order: PrintOrderView,
    pub line_items: Vec<PrintLineItemView>,
    pub printed_at: String,
}

/// Print order invoice or packing slip.
#[instrument(skip(_admin, state))]
pub async fn print(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<PrintQuery>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state.shopify().get_order(&order_id).await {
        Ok(Some(order)) => {
            let printed_at = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();

            let print_order = PrintOrderView {
                name: order.name.clone(),
                created_at: order.created_at.clone(),
                financial_status: order
                    .financial_status
                    .as_ref()
                    .map_or_else(|| "Pending".to_string(), |s| format!("{s:?}")),
                subtotal: format_price(&order.subtotal_price),
                shipping: format_price(&order.total_shipping_price),
                discount: format_price(&order.total_discounts),
                tax: format_price(&order.total_tax),
                total: format_price(&order.total_price),
                note: order.note.clone(),
                shipping_method: None, // Would come from shipping lines
                shipping_address: order.shipping_address.as_ref().map(AddressView::from),
                billing_address: order.billing_address.as_ref().map(AddressView::from),
            };

            let line_items: Vec<PrintLineItemView> = order
                .line_items
                .iter()
                .map(PrintLineItemView::from)
                .collect();

            let doc_type = query.doc_type.as_deref().unwrap_or("invoice");

            if doc_type == "packing_slip" {
                let template = OrderPackingSlipTemplate {
                    order: print_order,
                    line_items,
                    printed_at,
                };
                Html(template.render().unwrap_or_else(|e| {
                    tracing::error!("Template render error: {}", e);
                    "Internal Server Error".to_string()
                }))
                .into_response()
            } else {
                let template = OrderInvoiceTemplate {
                    order: print_order,
                    line_items,
                    printed_at,
                };
                Html(template.render().unwrap_or_else(|e| {
                    tracing::error!("Template render error: {}", e);
                    "Internal Server Error".to_string()
                }))
                .into_response()
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Order not found").into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch order for printing: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch order").into_response()
        }
    }
}

// =============================================================================
// Order Edit Handlers
// =============================================================================

/// View model for a calculated line item in the edit form.
#[derive(Debug, Clone)]
pub struct EditLineItemView {
    /// Line item ID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// Variant title.
    pub variant_title: Option<String>,
    /// SKU.
    pub sku: Option<String>,
    /// Current quantity.
    pub quantity: i64,
    /// Quantity that can be edited.
    pub editable_quantity: i64,
    /// Original quantity before edits.
    pub original_quantity: i64,
    /// Whether quantity changed.
    pub quantity_changed: bool,
    /// Whether this is a newly added item.
    pub is_new: bool,
    /// Original unit price formatted.
    pub original_unit_price: String,
    /// Discounted unit price formatted.
    pub discounted_unit_price: String,
    /// Line subtotal formatted.
    pub subtotal: String,
    /// Image URL.
    pub image_url: Option<String>,
    /// Whether item has a discount.
    pub has_discount: bool,
}

impl From<&CalculatedLineItem> for EditLineItemView {
    fn from(item: &CalculatedLineItem) -> Self {
        let original_price: f64 = item.original_unit_price.amount.parse().unwrap_or(0.0);
        let discounted_price: f64 = item.discounted_unit_price.amount.parse().unwrap_or(0.0);
        let subtotal: f64 = item.editable_subtotal.amount.parse().unwrap_or(0.0);

        Self {
            id: item.id.clone(),
            title: item.title.clone(),
            variant_title: item.variant_title.clone(),
            sku: item.sku.clone(),
            quantity: item.quantity,
            editable_quantity: item.editable_quantity,
            original_quantity: item.editable_quantity_before_changes,
            quantity_changed: item.editable_quantity != item.editable_quantity_before_changes,
            is_new: item.editable_quantity_before_changes == 0,
            original_unit_price: format!("${original_price:.2}"),
            discounted_unit_price: format!("${discounted_price:.2}"),
            subtotal: format!("${subtotal:.2}"),
            image_url: item.image.as_ref().map(|i| i.url.clone()),
            has_discount: item.has_staged_line_item_discount,
        }
    }
}

/// View model for a calculated shipping line in the edit form.
#[derive(Debug, Clone)]
pub struct EditShippingLineView {
    /// Shipping line ID.
    pub id: Option<String>,
    /// Shipping method title.
    pub title: String,
    /// Price formatted.
    pub price: String,
    /// Whether this was added during edit.
    pub is_new: bool,
    /// Whether this was removed during edit.
    pub is_removed: bool,
}

impl From<&CalculatedShippingLine> for EditShippingLineView {
    fn from(line: &CalculatedShippingLine) -> Self {
        let price: f64 = line.price.amount.parse().unwrap_or(0.0);
        Self {
            id: line.id.clone(),
            title: line.title.clone(),
            price: format!("${price:.2}"),
            is_new: line.staged_status == CalculatedShippingLineStagedStatus::Added,
            is_removed: line.staged_status == CalculatedShippingLineStagedStatus::Removed,
        }
    }
}

/// View model for the order edit page.
#[derive(Debug, Clone)]
pub struct OrderEditView {
    /// Calculated order ID (for mutations).
    pub calculated_order_id: String,
    /// Original order ID.
    pub order_id: String,
    /// Original order name.
    pub order_name: String,
    /// Existing line items.
    pub line_items: Vec<EditLineItemView>,
    /// Newly added line items.
    pub added_line_items: Vec<EditLineItemView>,
    /// Shipping lines.
    pub shipping_lines: Vec<EditShippingLineView>,
    /// Original subtotal formatted.
    pub original_subtotal: String,
    /// New subtotal formatted.
    pub new_subtotal: String,
    /// Original total formatted.
    pub original_total: String,
    /// New total formatted.
    pub new_total: String,
    /// Amount customer owes or will be refunded.
    pub total_outstanding: String,
    /// Whether customer owes money (positive outstanding).
    pub customer_owes: bool,
    /// Whether customer will be refunded (negative outstanding).
    pub customer_refund: bool,
    /// Total items quantity.
    pub total_items: i64,
    /// Whether there are any changes.
    pub has_changes: bool,
}

impl From<&CalculatedOrder> for OrderEditView {
    fn from(order: &CalculatedOrder) -> Self {
        let subtotal: f64 = order.subtotal_price.amount.parse().unwrap_or(0.0);
        let total: f64 = order.total_price.amount.parse().unwrap_or(0.0);
        let outstanding: f64 = order.total_outstanding.amount.parse().unwrap_or(0.0);

        Self {
            calculated_order_id: order.id.clone(),
            order_id: order.original_order_id.clone(),
            order_name: order.original_order_name.clone(),
            line_items: order
                .line_items
                .iter()
                .map(EditLineItemView::from)
                .collect(),
            added_line_items: order
                .added_line_items
                .iter()
                .map(EditLineItemView::from)
                .collect(),
            shipping_lines: order
                .shipping_lines
                .iter()
                .map(EditShippingLineView::from)
                .collect(),
            original_subtotal: format!("${subtotal:.2}"),
            new_subtotal: format!("${subtotal:.2}"),
            original_total: format!("${total:.2}"),
            new_total: format!("${total:.2}"),
            total_outstanding: format!("${:.2}", outstanding.abs()),
            customer_owes: outstanding > 0.0,
            customer_refund: outstanding < 0.0,
            total_items: order.subtotal_line_items_quantity,
            has_changes: order.has_changes(),
        }
    }
}

/// Template for the order edit page.
#[derive(Template)]
#[template(path = "orders/edit.html")]
pub struct OrderEditTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub edit: OrderEditView,
}

/// Template for the edit line items partial (HTMX swap).
#[derive(Template)]
#[template(path = "orders/_edit_line_items.html")]
pub struct EditLineItemsPartial {
    pub edit: OrderEditView,
}

/// Template for the edit summary partial (HTMX swap).
#[derive(Template)]
#[template(path = "orders/_edit_summary.html")]
pub struct EditSummaryPartial {
    pub edit: OrderEditView,
}

/// Input for adding a variant to the order.
#[derive(Debug, Deserialize)]
pub struct AddVariantInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Variant ID to add.
    pub variant_id: String,
    /// Quantity to add.
    pub quantity: i64,
}

/// Input for adding a custom item.
#[derive(Debug, Deserialize)]
pub struct AddCustomItemInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Item title.
    pub title: String,
    /// Quantity.
    pub quantity: i64,
    /// Price amount (decimal string).
    pub price: String,
    /// Whether taxable.
    #[serde(default)]
    pub taxable: bool,
    /// Whether requires shipping.
    #[serde(default)]
    pub requires_shipping: bool,
}

/// Input for setting line item quantity.
#[derive(Debug, Deserialize)]
pub struct SetQuantityInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Line item ID.
    pub line_item_id: String,
    /// New quantity.
    pub quantity: i64,
    /// Whether to restock.
    #[serde(default)]
    pub restock: bool,
}

/// Input for adding a discount.
#[derive(Debug, Deserialize)]
pub struct AddDiscountInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Line item ID.
    pub line_item_id: String,
    /// Discount type (percent or fixed).
    pub discount_type: String,
    /// Discount value.
    pub value: String,
    /// Optional description.
    pub description: Option<String>,
}

/// Input for updating a discount.
#[derive(Debug, Deserialize)]
pub struct UpdateDiscountInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Discount application ID.
    pub discount_application_id: String,
    /// Discount type (percent or fixed).
    pub discount_type: String,
    /// Discount value.
    pub value: String,
    /// Optional description.
    pub description: Option<String>,
}

/// Input for removing a discount.
#[derive(Debug, Deserialize)]
pub struct RemoveDiscountInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Discount application ID.
    pub discount_application_id: String,
}

/// Input for adding a shipping line.
#[derive(Debug, Deserialize)]
pub struct AddShippingInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Shipping method title.
    pub title: String,
    /// Price amount.
    pub price: String,
}

/// Input for updating a shipping line.
#[derive(Debug, Deserialize)]
pub struct UpdateShippingInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Shipping line ID.
    pub shipping_line_id: String,
    /// New title.
    pub title: Option<String>,
    /// New price.
    pub price: Option<String>,
}

/// Input for removing a shipping line.
#[derive(Debug, Deserialize)]
pub struct RemoveShippingInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Shipping line ID.
    pub shipping_line_id: String,
}

/// Input for committing order edit.
#[derive(Debug, Deserialize)]
pub struct CommitEditInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Whether to notify customer.
    #[serde(default)]
    pub notify_customer: bool,
    /// Optional staff note.
    pub staff_note: Option<String>,
}

/// Input for product search.
#[derive(Debug, Deserialize)]
pub struct ProductSearchQuery {
    /// Search query.
    pub q: Option<String>,
}

/// Start editing an order.
#[instrument(skip(admin, state))]
pub async fn edit(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Extract short ID for URL paths
    let short_id = if id.starts_with("gid://") {
        id.split('/').next_back().unwrap_or(&id).to_string()
    } else {
        id.clone()
    };

    let order_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state.shopify().order_edit_begin(&order_id).await {
        Ok(calculated_order) => {
            let edit = OrderEditView::from(&calculated_order);
            let template = OrderEditTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: format!("/orders/{short_id}/edit"),
                edit,
            };
            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to begin order edit");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to begin order edit: {e}"),
            )
                .into_response()
        }
    }
}

/// Add a variant to the order edit (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_add_variant(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<AddVariantInput>,
) -> impl IntoResponse {
    let variant_id = if input.variant_id.starts_with("gid://") {
        input.variant_id
    } else {
        format!("gid://shopify/ProductVariant/{}", input.variant_id)
    };

    match state
        .shopify()
        .order_edit_add_variant(&input.calculated_order_id, &variant_id, input.quantity)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, variant_id = %variant_id, "Added variant to order edit");
            // Redirect to refresh the edit page
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to add variant");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to add variant: {e}"),
            )
                .into_response()
        }
    }
}

/// Add a custom item to the order edit (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_add_custom_item(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<AddCustomItemInput>,
) -> impl IntoResponse {
    let price = Money {
        amount: input.price,
        currency_code: "USD".to_string(),
    };

    match state
        .shopify()
        .order_edit_add_custom_item(
            &input.calculated_order_id,
            &input.title,
            input.quantity,
            &price,
            input.taxable,
            input.requires_shipping,
        )
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, title = %input.title, "Added custom item to order edit");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to add custom item");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to add custom item: {e}"),
            )
                .into_response()
        }
    }
}

/// Set line item quantity in order edit (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_set_quantity(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<SetQuantityInput>,
) -> impl IntoResponse {
    match state
        .shopify()
        .order_edit_set_quantity(
            &input.calculated_order_id,
            &input.line_item_id,
            input.quantity,
            input.restock,
        )
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, line_item_id = %input.line_item_id, quantity = %input.quantity, "Updated quantity");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to set quantity");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to set quantity: {e}"),
            )
                .into_response()
        }
    }
}

/// Add a discount to a line item (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_add_discount(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<AddDiscountInput>,
) -> impl IntoResponse {
    let discount = if input.discount_type == "percent" {
        let percent: f64 = input.value.parse().unwrap_or(0.0);
        OrderEditAppliedDiscountInput::percentage(percent, input.description)
    } else {
        let amount: f64 = input.value.parse().unwrap_or(0.0);
        let money = Money {
            amount: format!("{amount:.2}"),
            currency_code: "USD".to_string(),
        };
        OrderEditAppliedDiscountInput::fixed_amount(money, input.description)
    };

    match state
        .shopify()
        .order_edit_add_line_item_discount(
            &input.calculated_order_id,
            &input.line_item_id,
            &discount,
        )
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, "Added discount to line item");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to add discount");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to add discount: {e}"),
            )
                .into_response()
        }
    }
}

/// Update a discount (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_update_discount(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<UpdateDiscountInput>,
) -> impl IntoResponse {
    let discount = if input.discount_type == "percent" {
        let percent: f64 = input.value.parse().unwrap_or(0.0);
        OrderEditAppliedDiscountInput::percentage(percent, input.description)
    } else {
        let amount: f64 = input.value.parse().unwrap_or(0.0);
        let money = Money {
            amount: format!("{amount:.2}"),
            currency_code: "USD".to_string(),
        };
        OrderEditAppliedDiscountInput::fixed_amount(money, input.description)
    };

    match state
        .shopify()
        .order_edit_update_discount(
            &input.calculated_order_id,
            &input.discount_application_id,
            &discount,
        )
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, "Updated discount");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to update discount");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to update discount: {e}"),
            )
                .into_response()
        }
    }
}

/// Remove a discount (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_remove_discount(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<RemoveDiscountInput>,
) -> impl IntoResponse {
    match state
        .shopify()
        .order_edit_remove_discount(&input.calculated_order_id, &input.discount_application_id)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, "Removed discount");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to remove discount");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to remove discount: {e}"),
            )
                .into_response()
        }
    }
}

/// Add a shipping line (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_add_shipping(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<AddShippingInput>,
) -> impl IntoResponse {
    let shipping_input = OrderEditAddShippingLineInput {
        title: input.title,
        price: Money {
            amount: input.price,
            currency_code: "USD".to_string(),
        },
    };

    match state
        .shopify()
        .order_edit_add_shipping_line(&input.calculated_order_id, &shipping_input)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, "Added shipping line");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to add shipping line");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to add shipping line: {e}"),
            )
                .into_response()
        }
    }
}

/// Update a shipping line (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_update_shipping(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<UpdateShippingInput>,
) -> impl IntoResponse {
    let shipping_input = OrderEditUpdateShippingLineInput {
        title: input.title,
        price: input.price.map(|p| Money {
            amount: p,
            currency_code: "USD".to_string(),
        }),
    };

    match state
        .shopify()
        .order_edit_update_shipping_line(
            &input.calculated_order_id,
            &input.shipping_line_id,
            &shipping_input,
        )
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, "Updated shipping line");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to update shipping line");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to update shipping line: {e}"),
            )
                .into_response()
        }
    }
}

/// Remove a shipping line (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_remove_shipping(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<RemoveShippingInput>,
) -> impl IntoResponse {
    match state
        .shopify()
        .order_edit_remove_shipping_line(&input.calculated_order_id, &input.shipping_line_id)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, "Removed shipping line");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to remove shipping line");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to remove shipping line: {e}"),
            )
                .into_response()
        }
    }
}

/// Commit the order edit.
#[instrument(skip(_admin, state))]
pub async fn edit_commit(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<CommitEditInput>,
) -> impl IntoResponse {
    match state
        .shopify()
        .order_edit_commit(
            &input.calculated_order_id,
            input.notify_customer,
            input.staff_note.as_deref(),
        )
        .await
    {
        Ok(order_id) => {
            tracing::info!(order_id = %order_id, "Order edit committed");
            // Extract short ID for redirect
            let short_id = order_id.strip_prefix("gid://shopify/Order/").unwrap_or(&id);
            Redirect::to(&format!("/orders/{short_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to commit order edit");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to commit order edit: {e}"),
            )
                .into_response()
        }
    }
}

/// Discard the order edit and return to order detail.
#[instrument(skip(_admin))]
pub async fn edit_discard(
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Simply redirect back to the order detail page
    // The calculated order session will expire automatically
    Redirect::to(&format!("/orders/{id}"))
}

/// Search products for adding to order (HTMX).
#[instrument(skip(state))]
pub async fn edit_search_products(
    RequireAdminAuth(_): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<ProductSearchQuery>,
) -> impl IntoResponse {
    use std::fmt::Write;

    let search_query = query.q.filter(|q| !q.is_empty());

    match state.shopify().get_products(20, None, search_query).await {
        Ok(products) => {
            // Build a simple HTML list of products with variants
            let mut html = String::new();
            for product in products.products {
                let _ = write!(
                    html,
                    r#"<div class="p-3 border-b border-border hover:bg-muted/50">
                        <div class="font-medium">{}</div>
                        <div class="mt-2 space-y-1">"#,
                    product.title
                );
                for variant in &product.variants {
                    let price: f64 = variant.price.amount.parse().unwrap_or(0.0);
                    let variant_title = &variant.title;
                    let sku_display = variant
                        .sku
                        .as_ref()
                        .filter(|s| !s.is_empty())
                        .map_or(String::new(), |s| format!("({s})"));
                    let _ = write!(
                        html,
                        r##"<button type="button"
                            class="w-full text-left px-2 py-1 rounded hover:bg-primary/10 text-sm flex justify-between items-center"
                            hx-post="/orders/{id}/edit/add-variant"
                            hx-vals='{{"variant_id": "{}", "quantity": 1}}'
                            hx-target="#edit-content"
                            hx-swap="outerHTML">
                            <span>{variant_title} {sku_display}</span>
                            <span class="text-muted-foreground">${price:.2}</span>
                        </button>"##,
                        variant.id,
                    );
                }
                html.push_str("</div></div>");
            }
            if html.is_empty() {
                html =
                    r#"<div class="p-4 text-center text-muted-foreground">No products found</div>"#
                        .to_string();
            }
            Html(html).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to search products");
            Html(
                r#"<div class="p-4 text-center text-destructive">Failed to search products</div>"#
                    .to_string(),
            )
            .into_response()
        }
    }
}

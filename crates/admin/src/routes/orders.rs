//! Orders management route handlers.

use askama::Template;
use axum::{
    Form,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use serde::Deserialize;
use tracing::instrument;

use crate::{
    components::data_table::{
        BulkAction, FilterType, TableColumn, TableFilter, orders_table_config,
    },
    filters,
    middleware::auth::RequireAdminAuth,
    shopify::types::{
        Address, DeliveryCategory, FinancialStatus, Fulfillment, FulfillmentStatus, Money, Order,
        OrderLineItem, OrderListItem, OrderReturnStatus, OrderRiskLevel, OrderSortKey,
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
    pub line1: String,
    pub line2: Option<String>,
    pub city_state_zip: String,
    pub country: String,
    pub phone: Option<String>,
}

impl From<&Address> for AddressView {
    fn from(addr: &Address) -> Self {
        let first = addr.first_name.as_deref().unwrap_or("");
        let last = addr.last_name.as_deref().unwrap_or("");
        let name = format!("{first} {last}").trim().to_string();

        let line1 = addr.address1.clone().unwrap_or_default();
        let line2 = addr.address2.clone().filter(|s| !s.is_empty());

        let city = addr.city.as_deref().unwrap_or("");
        let state = addr.province_code.as_deref().unwrap_or("");
        let zip = addr.zip.as_deref().unwrap_or("");
        let city_state_zip = format!("{city}, {state} {zip}").trim().to_string();

        let country = addr.country_code.clone().unwrap_or_default();

        Self {
            name: if name.is_empty() {
                "N/A".to_string()
            } else {
                name
            },
            line1,
            line2,
            city_state_zip,
            country,
            phone: addr.phone.clone(),
        }
    }
}

/// Fulfillment view for templates.
#[derive(Debug, Clone)]
pub struct FulfillmentView {
    pub id: String,
    pub status: String,
    pub tracking_number: Option<String>,
    pub tracking_url: Option<String>,
    pub carrier: Option<String>,
    pub created_at: String,
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
            created_at: f.created_at.clone(),
        }
    }
}

/// Order detail view for templates.
#[derive(Debug, Clone)]
pub struct OrderDetailView {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub customer_name: String,
    pub customer_email: Option<String>,
    pub customer_phone: Option<String>,
    pub note: Option<String>,
    pub fulfillment_status: String,
    pub fulfillment_status_class: String,
    pub financial_status: String,
    pub financial_status_class: String,
    pub is_paid: bool,
    pub is_test: bool,
    pub subtotal: String,
    pub shipping: String,
    pub tax: String,
    pub discount: String,
    pub total: String,
    pub line_items: Vec<LineItemView>,
    pub fulfillments: Vec<FulfillmentView>,
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
        let (fulfillment_status, fulfillment_status_class) = fulfillment_status_display(order);
        let (financial_status, financial_status_class, is_paid) = financial_status_display(order);

        Self {
            id: order.id.clone(),
            name: order.name.clone(),
            created_at: order.created_at.clone(),
            customer_name: get_customer_name(order),
            customer_email: order.email.clone(),
            customer_phone: order.phone.clone(),
            note: order.note.clone(),
            fulfillment_status,
            fulfillment_status_class,
            financial_status,
            financial_status_class,
            is_paid,
            is_test: order.test,
            subtotal: format_price(&order.subtotal_price),
            shipping: format_price(&order.total_shipping_price),
            tax: format_price(&order.total_tax),
            discount: format_price(&order.total_discounts),
            total: format_price(&order.total_price),
            line_items: order.line_items.iter().map(LineItemView::from).collect(),
            fulfillments: order
                .fulfillments
                .iter()
                .map(FulfillmentView::from)
                .collect(),
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

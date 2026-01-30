//! Type definitions and conversions for order views.

use serde::Deserialize;

use crate::shopify::types::{
    Address, CalculatedLineItem, CalculatedOrder, CalculatedShippingLine,
    CalculatedShippingLineStagedStatus, FinancialStatus, Fulfillment, FulfillmentStatus, Money,
    Order, OrderLineItem, OrderListItem, OrderReturnStatus, OrderRiskLevel,
};

use super::super::dashboard::AdminUserView;

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

// =============================================================================
// Table View Types
// =============================================================================

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
// Helper Functions
// =============================================================================

/// Extract numeric ID from Shopify GID.
#[must_use]
pub fn extract_numeric_id(gid: &str) -> String {
    gid.split('/').next_back().unwrap_or(gid).to_string()
}

/// Format a Shopify Money type as a price string.
#[must_use]
pub fn format_price(money: &Money) -> String {
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
#[must_use]
pub fn format_fulfillment_status(status: Option<&FulfillmentStatus>) -> (String, String) {
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
#[must_use]
pub fn get_customer_name(order: &Order) -> String {
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
#[must_use]
pub fn build_shopify_query(query: &OrdersQuery) -> Option<String> {
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
#[must_use]
pub fn build_preserve_params(query: &OrdersQuery) -> String {
    let mut params = Vec::new();

    if let Some(q) = &query.query
        && !q.is_empty()
    {
        params.push(format!("query={}", urlencoding::encode(q)));
    }
    // Note: sort and dir are intentionally excluded here because they are
    // set explicitly in the sort column header links. Including them would
    // create duplicate query parameters.
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
#[must_use]
pub fn fulfillment_status_display(order: &Order) -> (String, String) {
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

// =============================================================================
// Order Detail Views
// =============================================================================

/// Lot allocation view for line items.
#[derive(Debug, Clone)]
pub struct LineItemAllocationView {
    /// Allocation ID.
    pub id: i32,
    /// Lot ID.
    pub lot_id: i32,
    /// Lot number display.
    pub lot_number: String,
    /// Batch number display.
    pub batch_number: String,
    /// Quantity allocated from this lot.
    pub quantity: i32,
    /// Cost per unit from the lot's batch.
    pub cost_per_unit: String,
    /// When the allocation was made.
    pub allocated_at: String,
}

/// Available lot for allocation selection.
#[derive(Debug, Clone)]
pub struct AvailableLotView {
    /// Lot ID.
    pub id: i32,
    /// Lot number.
    pub lot_number: String,
    /// Batch number.
    pub batch_number: String,
    /// Quantity remaining.
    pub quantity_remaining: i64,
    /// Cost per unit formatted.
    pub cost_per_unit: String,
    /// Received date formatted.
    pub received_date: String,
}

/// Line item view for templates.
#[derive(Debug, Clone)]
pub struct LineItemView {
    pub id: String,
    pub title: String,
    pub variant_title: Option<String>,
    pub sku: Option<String>,
    pub quantity: i64,
    pub unit_price: String,
    pub total_price: String,
    /// Shopify product GID for lot matching.
    pub product_id: Option<String>,
    /// Shopify variant GID.
    pub variant_id: Option<String>,
    /// Lot allocations for this line item.
    pub allocations: Vec<LineItemAllocationView>,
    /// Total quantity allocated from lots.
    pub allocated_quantity: i64,
    /// Quantity still needing allocation.
    pub needed_quantity: i64,
    /// Whether fully allocated.
    pub is_fully_allocated: bool,
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
            id: item.id.clone(),
            title: item.title.clone(),
            variant_title: item.variant_title.clone(),
            sku: item.sku.clone(),
            quantity: item.quantity,
            unit_price,
            total_price: format!("${total:.2}"),
            product_id: item.product_id.clone(),
            variant_id: item.variant_id.clone(),
            allocations: vec![],
            allocated_quantity: 0,
            needed_quantity: item.quantity,
            is_fully_allocated: false,
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
    /// Shopify product GID for lot matching.
    pub product_id: Option<String>,
    /// Shopify variant GID.
    pub variant_id: Option<String>,
    /// Lot allocations for this line item.
    pub allocations: Vec<LineItemAllocationView>,
    /// Total quantity allocated from lots.
    pub allocated_quantity: i64,
    /// Quantity still needing allocation (for template compatibility with `LineItemView`).
    pub needed_quantity: i64,
    /// Alias for `total_quantity` (for template compatibility with `LineItemView`).
    pub quantity: i64,
    /// Whether fully allocated.
    pub is_fully_allocated: bool,
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
#[must_use]
pub fn financial_status_display(order: &Order) -> (String, String, bool) {
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

/// Convert a fulfillment order to view with product ID lookup.
fn convert_fulfillment_order_to_view(
    fo: &crate::shopify::types::FulfillmentOrderDetail,
    line_item_map: &std::collections::HashMap<&str, (&Option<String>, &Option<String>)>,
) -> FulfillmentOrderView {
    let line_items = fo
        .line_items
        .iter()
        .map(|li| {
            let (product_id, variant_id) = line_item_map
                .get(li.line_item_id.as_str())
                .map_or((None, None), |(pid, vid)| ((*pid).clone(), (*vid).clone()));
            FulfillmentOrderLineItemView {
                // Use line_item_id as id since allocations are stored against it
                id: li.line_item_id.clone(),
                title: li.title.clone(),
                variant_title: li.variant_title.clone(),
                sku: li.sku.clone(),
                image: li.image.as_ref().map(|img| img.url.clone()),
                total_quantity: li.total_quantity,
                remaining_quantity: li.remaining_quantity,
                product_id,
                variant_id,
                allocations: vec![],
                allocated_quantity: 0,
                needed_quantity: li.remaining_quantity,
                quantity: li.remaining_quantity,
                is_fully_allocated: false,
            }
        })
        .collect();
    FulfillmentOrderView {
        id: fo.id.clone(),
        status: fo.status.clone(),
        location_name: fo.location_name.clone(),
        line_items,
    }
}

/// Convert fulfillment orders to views with product ID lookup from line items.
fn convert_fulfillment_orders(order: &Order) -> Vec<FulfillmentOrderView> {
    // Build a mapping from line_item_id -> (product_id, variant_id)
    let line_item_map: std::collections::HashMap<&str, (&Option<String>, &Option<String>)> = order
        .line_items
        .iter()
        .map(|li| (li.id.as_str(), (&li.product_id, &li.variant_id)))
        .collect();

    order
        .fulfillment_orders
        .iter()
        .map(|fo| convert_fulfillment_order_to_view(fo, &line_item_map))
        .collect()
}

impl From<&Order> for OrderDetailView {
    fn from(order: &Order) -> Self {
        let short_id = extract_numeric_id(&order.id);
        let (fulfillment_status, fulfillment_status_class) = fulfillment_status_display(order);
        let (financial_status, financial_status_class, is_paid) = financial_status_display(order);
        let total_str = format_price(&order.total_price);
        let fulfillment_orders = convert_fulfillment_orders(order);

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
            fulfillment_orders,
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

// =============================================================================
// Order Edit Views
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

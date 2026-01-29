//! Order domain types for Shopify Admin API.

use serde::{Deserialize, Serialize};

use super::common::{Address, Image, Money, PageInfo};
use super::customer::MarketingConsent;

// =============================================================================
// Order Status Types
// =============================================================================

/// Order financial status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinancialStatus {
    /// No payment authorized.
    Pending,
    /// Payment has been authorized but not captured.
    Authorized,
    /// Payment has been captured.
    Paid,
    /// Payment has been partially paid.
    PartiallyPaid,
    /// Payment has been refunded.
    Refunded,
    /// Payment has been partially refunded.
    PartiallyRefunded,
    /// Payment has been voided.
    Voided,
    /// Payment has expired.
    Expired,
}

/// Order fulfillment status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FulfillmentStatus {
    /// No items have been fulfilled.
    Unfulfilled,
    /// Some items have been fulfilled.
    PartiallyFulfilled,
    /// All items have been fulfilled.
    Fulfilled,
    /// Fulfillment is on hold.
    OnHold,
    /// Items are being prepared.
    InProgress,
    /// Order was restocked.
    Restocked,
    /// Scheduled for fulfillment.
    Scheduled,
    /// Pending fulfillment.
    PendingFulfillment,
    /// Order is open.
    Open,
    /// Fulfillment request was declined.
    RequestDeclined,
}

/// Order return status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderReturnStatus {
    /// No return.
    NoReturn,
    /// Return requested by customer.
    ReturnRequested,
    /// Return in progress.
    InProgress,
    /// Return completed.
    Returned,
}

impl std::fmt::Display for OrderReturnStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoReturn => write!(f, "No Return"),
            Self::ReturnRequested => write!(f, "Return Requested"),
            Self::InProgress => write!(f, "In Progress"),
            Self::Returned => write!(f, "Returned"),
        }
    }
}

/// Delivery category for shipping lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DeliveryCategory {
    /// Standard shipping delivery.
    Shipping,
    /// Local delivery.
    LocalDelivery,
    /// Store pickup.
    Pickup,
    /// Digital delivery (no physical shipping).
    None,
}

impl std::fmt::Display for DeliveryCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shipping => write!(f, "Shipping"),
            Self::LocalDelivery => write!(f, "Local Delivery"),
            Self::Pickup => write!(f, "Pickup"),
            Self::None => write!(f, "Digital"),
        }
    }
}

/// Order cancel reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderCancelReason {
    /// Customer changed mind.
    Customer,
    /// Fraudulent order.
    Fraud,
    /// Item out of stock.
    Inventory,
    /// Payment declined.
    Declined,
    /// Other reason.
    Other,
    /// Staff requested.
    StaffRequest,
}

impl std::fmt::Display for OrderCancelReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Customer => write!(f, "Customer changed/cancelled order"),
            Self::Fraud => write!(f, "Fraudulent order"),
            Self::Inventory => write!(f, "Items unavailable"),
            Self::Declined => write!(f, "Payment declined"),
            Self::Other => write!(f, "Other"),
            Self::StaffRequest => write!(f, "Staff request"),
        }
    }
}

// =============================================================================
// Tracking and Fulfillment Types
// =============================================================================

/// Tracking information for a fulfillment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingInfo {
    /// Tracking company/carrier name.
    pub company: Option<String>,
    /// Tracking number.
    pub number: Option<String>,
    /// Tracking URL.
    pub url: Option<String>,
}

/// A fulfillment for an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fulfillment {
    /// Fulfillment ID.
    pub id: String,
    /// Fulfillment status.
    pub status: String,
    /// Tracking information.
    pub tracking_info: Vec<TrackingInfo>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

/// A line item in a fulfillment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentLineItem {
    /// Fulfillment line item ID.
    pub id: String,
    /// Quantity fulfilled.
    pub quantity: i64,
    /// Original line item ID.
    pub line_item_id: String,
    /// Product title.
    pub title: String,
    /// Variant title.
    pub variant_title: Option<String>,
    /// SKU.
    pub sku: Option<String>,
    /// Image.
    pub image: Option<Image>,
}

/// Extended fulfillment for detail view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentDetail {
    /// Fulfillment ID.
    pub id: String,
    /// Fulfillment name.
    pub name: Option<String>,
    /// Status (raw string from API).
    pub status: String,
    /// Display status.
    pub display_status: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
    /// Tracking information.
    pub tracking_info: Vec<TrackingInfo>,
    /// Line items in this fulfillment.
    pub line_items: Vec<FulfillmentLineItem>,
}

/// Supported actions on a fulfillment order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FulfillmentOrderAction {
    /// Can create fulfillment.
    CreateFulfillment,
    /// Can request fulfillment.
    RequestFulfillment,
    /// Can cancel fulfillment request.
    CancelFulfillmentOrder,
    /// Can move fulfillment order.
    Move,
    /// Can hold fulfillment order.
    Hold,
    /// Can release hold.
    ReleaseHold,
    /// Can open fulfillment order.
    Open,
    /// Can close fulfillment order.
    Close,
    /// Can mark as open.
    MarkAsOpen,
    /// Can external fulfillment cancel.
    ExternalFulfillmentCancel,
    /// Can external fulfillment request.
    ExternalFulfillmentRequest,
}

/// A line item in a fulfillment order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentOrderLineItemDetail {
    /// Fulfillment order line item ID.
    pub id: String,
    /// Total quantity.
    pub total_quantity: i64,
    /// Remaining quantity to fulfill.
    pub remaining_quantity: i64,
    /// Original line item ID.
    pub line_item_id: String,
    /// Product title.
    pub title: String,
    /// Variant title.
    pub variant_title: Option<String>,
    /// SKU.
    pub sku: Option<String>,
    /// Image.
    pub image: Option<Image>,
}

/// A fulfillment order (group of items pending fulfillment).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentOrderDetail {
    /// Fulfillment order ID.
    pub id: String,
    /// Status.
    pub status: String,
    /// Request status.
    pub request_status: Option<String>,
    /// Location ID.
    pub location_id: Option<String>,
    /// Location name.
    pub location_name: Option<String>,
    /// Supported actions.
    pub supported_actions: Vec<FulfillmentOrderAction>,
    /// Line items in this fulfillment order.
    pub line_items: Vec<FulfillmentOrderLineItemDetail>,
}

// =============================================================================
// Line Item Types
// =============================================================================

/// A line item in an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderLineItem {
    /// Line item ID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// Variant title.
    pub variant_title: Option<String>,
    /// SKU.
    pub sku: Option<String>,
    /// Quantity ordered.
    pub quantity: i64,
    /// Price per unit.
    pub original_unit_price: Money,
    /// Discounted price per unit.
    pub discounted_unit_price: Money,
    /// Total discount amount.
    pub total_discount: Money,
    /// Product ID.
    pub product_id: Option<String>,
    /// Variant ID.
    pub variant_id: Option<String>,
    /// Whether requires shipping.
    pub requires_shipping: bool,
    /// Whether is a gift card.
    pub is_gift_card: bool,
}

/// Extended line item for order detail view with quantities and image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDetailLineItem {
    /// Line item ID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// Full name (title + variant).
    pub name: Option<String>,
    /// Variant title.
    pub variant_title: Option<String>,
    /// SKU.
    pub sku: Option<String>,
    /// Original quantity ordered.
    pub quantity: i64,
    /// Current quantity (after edits).
    pub current_quantity: i64,
    /// Quantity not yet fulfilled.
    pub unfulfilled_quantity: i64,
    /// Quantity that can be refunded.
    pub refundable_quantity: i64,
    /// Quantity that cannot be fulfilled (digital, etc).
    pub non_fulfillable_quantity: i64,
    /// Original price per unit.
    pub original_unit_price: Money,
    /// Discounted price per unit.
    pub discounted_unit_price: Money,
    /// Total discount amount.
    pub total_discount: Money,
    /// Original total (quantity × original price).
    pub original_total: Money,
    /// Discounted total (quantity × discounted price).
    pub discounted_total: Money,
    /// Product ID.
    pub product_id: Option<String>,
    /// Variant ID.
    pub variant_id: Option<String>,
    /// Line item image.
    pub image: Option<Image>,
    /// Whether requires shipping.
    pub requires_shipping: bool,
    /// Whether is a gift card.
    pub is_gift_card: bool,
    /// Whether taxable.
    pub taxable: bool,
}

// =============================================================================
// Transaction Types
// =============================================================================

/// Transaction kind for order payments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionKind {
    /// Authorization hold.
    Authorization,
    /// Capture of authorized funds.
    Capture,
    /// Sale (authorize + capture).
    Sale,
    /// Refund to customer.
    Refund,
    /// Void of authorization.
    Void,
    /// Pending transaction.
    Pending,
    /// Chargeback initiated.
    Chargeback,
    /// EMV authorization.
    EmvAuthorization,
    /// Suggested refund.
    SuggestedRefund,
    /// Change (cash payment).
    Change,
}

impl std::fmt::Display for TransactionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Authorization => write!(f, "Authorization"),
            Self::Capture => write!(f, "Capture"),
            Self::Sale => write!(f, "Sale"),
            Self::Refund => write!(f, "Refund"),
            Self::Void => write!(f, "Void"),
            Self::Pending => write!(f, "Pending"),
            Self::Chargeback => write!(f, "Chargeback"),
            Self::EmvAuthorization => write!(f, "EMV Authorization"),
            Self::SuggestedRefund => write!(f, "Suggested Refund"),
            Self::Change => write!(f, "Change"),
        }
    }
}

/// Transaction status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionStatus {
    /// Transaction succeeded.
    Success,
    /// Transaction is pending.
    Pending,
    /// Transaction failed.
    Failure,
    /// Transaction encountered an error.
    Error,
    /// Transaction was awaiting response.
    AwaitingResponse,
    /// Unknown status.
    Unknown,
}

impl std::fmt::Display for TransactionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "Success"),
            Self::Pending => write!(f, "Pending"),
            Self::Failure => write!(f, "Failure"),
            Self::Error => write!(f, "Error"),
            Self::AwaitingResponse => write!(f, "Awaiting Response"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Card payment details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardPaymentDetails {
    /// Card brand (Visa, Mastercard, etc.).
    pub company: Option<String>,
    /// Cardholder name.
    pub name: Option<String>,
    /// Last 4 digits.
    pub number: Option<String>,
    /// Card BIN (first 6 digits).
    pub bin: Option<String>,
    /// Expiration month.
    pub expiration_month: Option<i64>,
    /// Expiration year.
    pub expiration_year: Option<i64>,
    /// Digital wallet type (Apple Pay, Google Pay, etc.).
    pub wallet: Option<String>,
}

/// A payment transaction on an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderTransaction {
    /// Transaction ID.
    pub id: String,
    /// Transaction kind.
    pub kind: TransactionKind,
    /// Transaction status.
    pub status: TransactionStatus,
    /// Payment gateway name.
    pub gateway: Option<String>,
    /// When the transaction was created.
    pub created_at: String,
    /// When the transaction was processed.
    pub processed_at: Option<String>,
    /// Error code if failed.
    pub error_code: Option<String>,
    /// Transaction amount.
    pub amount: Money,
    /// Unsettled amount.
    pub total_unsettled: Option<Money>,
    /// Card payment details (if card payment).
    pub payment_details: Option<CardPaymentDetails>,
}

// =============================================================================
// Refund and Return Types
// =============================================================================

/// A line item in a refund.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundLineItem {
    /// Original line item ID.
    pub line_item_id: String,
    /// Product title.
    pub title: String,
    /// Variant title.
    pub variant_title: Option<String>,
    /// SKU.
    pub sku: Option<String>,
    /// Quantity refunded.
    pub quantity: i64,
    /// Whether the item was restocked.
    pub restocked: bool,
    /// Refund subtotal for this item.
    pub subtotal: Money,
}

/// A refund on an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRefund {
    /// Refund ID.
    pub id: String,
    /// When the refund was created.
    pub created_at: String,
    /// Refund note.
    pub note: Option<String>,
    /// Total refunded amount.
    pub total_refunded: Money,
    /// Line items included in this refund.
    pub line_items: Vec<RefundLineItem>,
    /// Transactions for this refund.
    pub transactions: Vec<OrderTransaction>,
}

/// Return status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReturnStatus {
    /// Return requested by customer.
    Requested,
    /// Return is open/in progress.
    Open,
    /// Return was canceled.
    Cancelled,
    /// Return is closed/completed.
    Closed,
    /// Return was declined.
    Declined,
}

impl std::fmt::Display for ReturnStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Requested => write!(f, "Requested"),
            Self::Open => write!(f, "Open"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Closed => write!(f, "Closed"),
            Self::Declined => write!(f, "Declined"),
        }
    }
}

/// A line item in a return.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnLineItem {
    /// Return line item ID.
    pub id: String,
    /// Quantity being returned.
    pub quantity: i64,
    /// Reason for return.
    pub return_reason: Option<String>,
    /// Customer note about the return.
    pub customer_note: Option<String>,
}

/// A return on an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderReturn {
    /// Return ID.
    pub id: String,
    /// Return name (e.g., "Return #1").
    pub name: Option<String>,
    /// Return status.
    pub status: ReturnStatus,
    /// When the return was created.
    pub created_at: String,
    /// Total quantity being returned.
    pub total_quantity: i64,
    /// Line items in this return.
    pub line_items: Vec<ReturnLineItem>,
}

// =============================================================================
// Risk and Event Types
// =============================================================================

/// Order risk level from fraud analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderRiskLevel {
    /// Low risk order.
    Low,
    /// Medium risk order.
    Medium,
    /// High risk order.
    High,
}

impl std::fmt::Display for OrderRiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High => write!(f, "High"),
        }
    }
}

/// Risk assessment for an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRisk {
    /// Risk level.
    pub level: OrderRiskLevel,
    /// Risk message/reason.
    pub message: Option<String>,
}

/// An event in the order timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderEvent {
    /// Event ID.
    pub id: String,
    /// When the event occurred.
    pub created_at: String,
    /// Event message.
    pub message: Option<String>,
    /// Whether attributed to an app.
    pub attribute_to_app: bool,
    /// Whether attributed to a user.
    pub attribute_to_user: bool,
    /// Whether this is a critical alert.
    pub critical_alert: bool,
    /// Author name (for comments).
    pub author_name: Option<String>,
}

// =============================================================================
// Shipping and Channel Types
// =============================================================================

/// Shipping line information for an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderShippingLine {
    /// Shipping method title.
    pub title: String,
    /// Delivery category.
    pub delivery_category: Option<DeliveryCategory>,
}

/// Extended shipping line for order detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShippingLineDetail {
    /// Shipping method title.
    pub title: String,
    /// Delivery category.
    pub delivery_category: Option<DeliveryCategory>,
    /// Discounted shipping price.
    pub discounted_price: Option<Money>,
}

/// Channel information for an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderChannelInfo {
    /// Channel name (e.g., "Online Store", "POS", "Shop").
    pub channel_name: Option<String>,
}

// =============================================================================
// Customer Info for Orders
// =============================================================================

/// Extended customer info for order detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDetailCustomer {
    /// Customer ID.
    pub id: String,
    /// Display name.
    pub display_name: String,
    /// Email.
    pub email: Option<String>,
    /// Phone.
    pub phone: Option<String>,
    /// Total orders count.
    pub orders_count: i64,
    /// Total amount spent.
    pub total_spent: Money,
    /// Customer note.
    pub note: Option<String>,
    /// Email marketing consent.
    pub email_marketing_consent: Option<MarketingConsent>,
}

// =============================================================================
// Order Types
// =============================================================================

/// An order in the admin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Order ID.
    pub id: String,
    /// Order name (e.g., "#1001").
    pub name: String,
    /// Order number.
    pub number: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
    /// Financial status.
    pub financial_status: Option<FinancialStatus>,
    /// Fulfillment status.
    pub fulfillment_status: Option<FulfillmentStatus>,
    /// Whether the order is fully paid.
    pub fully_paid: bool,
    /// Whether the order is test mode.
    pub test: bool,
    /// Customer email.
    pub email: Option<String>,
    /// Customer phone.
    pub phone: Option<String>,
    /// Order note.
    pub note: Option<String>,
    /// Subtotal price.
    pub subtotal_price: Money,
    /// Total shipping price.
    pub total_shipping_price: Money,
    /// Total tax.
    pub total_tax: Money,
    /// Total price.
    pub total_price: Money,
    /// Total discount amount.
    pub total_discounts: Money,
    /// Currency code.
    pub currency_code: String,
    /// Line items.
    pub line_items: Vec<OrderLineItem>,
    /// Fulfillments.
    pub fulfillments: Vec<Fulfillment>,
    /// Billing address.
    pub billing_address: Option<Address>,
    /// Shipping address.
    pub shipping_address: Option<Address>,
    /// Customer ID.
    pub customer_id: Option<String>,
}

/// Comprehensive order detail for the order detail page.
// Allow: Shopify API Order object has many independent boolean properties
// representing separate order states (fully_paid, confirmed, closed, test).
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDetail {
    /// Order ID.
    pub id: String,
    /// Order name (e.g., "#1001").
    pub name: String,
    /// Order number.
    pub number: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
    /// When order was closed.
    pub closed_at: Option<String>,
    /// When order was cancelled.
    pub cancelled_at: Option<String>,
    /// When order was processed.
    pub processed_at: Option<String>,
    /// Financial status.
    pub financial_status: Option<FinancialStatus>,
    /// Fulfillment status.
    pub fulfillment_status: Option<FulfillmentStatus>,
    /// Return status.
    pub return_status: Option<OrderReturnStatus>,
    /// Whether fully paid.
    pub fully_paid: bool,
    /// Whether order is confirmed.
    pub confirmed: bool,
    /// Whether order is closed.
    pub closed: bool,
    /// Cancel reason (if cancelled).
    pub cancel_reason: Option<OrderCancelReason>,
    /// Whether test order.
    pub test: bool,
    /// Customer email.
    pub email: Option<String>,
    /// Customer phone.
    pub phone: Option<String>,
    /// Order note.
    pub note: Option<String>,
    /// Order tags.
    pub tags: Vec<String>,
    /// Currency code.
    pub currency_code: String,
    // Financial summaries
    /// Subtotal price.
    pub subtotal_price: Money,
    /// Total shipping price.
    pub total_shipping_price: Money,
    /// Total tax.
    pub total_tax: Money,
    /// Total price.
    pub total_price: Money,
    /// Total discounts.
    pub total_discounts: Money,
    /// Current total price (after edits).
    pub current_total_price: Money,
    /// Outstanding balance.
    pub total_outstanding: Money,
    /// Total refunded.
    pub total_refunded: Money,
    /// Total capturable (authorized but not captured).
    pub total_capturable: Money,
    /// Net payment (paid minus refunded).
    pub net_payment: Money,
    // Related data
    /// Customer info.
    pub customer: Option<OrderDetailCustomer>,
    /// Billing address.
    pub billing_address: Option<Address>,
    /// Shipping address.
    pub shipping_address: Option<Address>,
    /// Shipping line.
    pub shipping_line: Option<ShippingLineDetail>,
    /// Applied discount codes.
    pub discount_codes: Vec<String>,
    /// Line items.
    pub line_items: Vec<OrderDetailLineItem>,
    /// Fulfillments.
    pub fulfillments: Vec<FulfillmentDetail>,
    /// Fulfillment orders (pending fulfillment).
    pub fulfillment_orders: Vec<FulfillmentOrderDetail>,
    /// Payment transactions.
    pub transactions: Vec<OrderTransaction>,
    /// Refunds.
    pub refunds: Vec<OrderRefund>,
    /// Returns.
    pub returns: Vec<OrderReturn>,
    /// Risk assessments.
    pub risks: Vec<OrderRisk>,
    /// Timeline events.
    pub events: Vec<OrderEvent>,
    /// Channel info.
    pub channel_info: Option<OrderChannelInfo>,
}

/// Extended order with list view fields.
// Allow: Shopify API Order object has independent boolean properties
// (fully_paid, cancelled, closed, test) that represent separate order states.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderListItem {
    /// Order ID.
    pub id: String,
    /// Order name (e.g., "#1001").
    pub name: String,
    /// Order number.
    pub number: i64,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
    /// When order was closed/archived.
    pub closed_at: Option<String>,
    /// When order was cancelled.
    pub cancelled_at: Option<String>,
    /// Financial status.
    pub financial_status: Option<FinancialStatus>,
    /// Fulfillment status.
    pub fulfillment_status: Option<FulfillmentStatus>,
    /// Return status.
    pub return_status: Option<OrderReturnStatus>,
    /// Whether the order is fully paid.
    pub fully_paid: bool,
    /// Whether order is cancelled.
    pub cancelled: bool,
    /// Whether order is closed/archived.
    pub closed: bool,
    /// Whether the order is test mode.
    pub test: bool,
    /// Customer email.
    pub email: Option<String>,
    /// Customer phone.
    pub phone: Option<String>,
    /// Order note.
    pub note: Option<String>,
    /// Order tags.
    pub tags: Vec<String>,
    /// Subtotal price.
    pub subtotal_price: Money,
    /// Total shipping price.
    pub total_shipping_price: Money,
    /// Total tax.
    pub total_tax: Money,
    /// Total price.
    pub total_price: Money,
    /// Total discount amount.
    pub total_discounts: Money,
    /// Currency code.
    pub currency_code: String,
    /// Line items (limited for list view).
    pub line_items: Vec<OrderLineItem>,
    /// Total line item quantity.
    pub total_items_quantity: i64,
    /// Fulfillments.
    pub fulfillments: Vec<Fulfillment>,
    /// Billing address.
    pub billing_address: Option<Address>,
    /// Shipping address.
    pub shipping_address: Option<Address>,
    /// Customer ID.
    pub customer_id: Option<String>,
    /// Customer display name.
    pub customer_name: Option<String>,
    /// Order risk assessments.
    pub risks: Vec<OrderRisk>,
    /// Channel information.
    pub channel_info: Option<OrderChannelInfo>,
    /// Shipping line.
    pub shipping_line: Option<OrderShippingLine>,
    /// Applied discount codes.
    pub discount_codes: Vec<String>,
}

// =============================================================================
// Sort Keys
// =============================================================================

/// Sort keys for order queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderSortKey {
    /// Sort by order number.
    OrderNumber,
    /// Sort by total price.
    TotalPrice,
    /// Sort by creation date.
    #[default]
    CreatedAt,
    /// Sort by processed date.
    ProcessedAt,
    /// Sort by last update.
    UpdatedAt,
    /// Sort by customer name.
    CustomerName,
    /// Sort by financial status.
    FinancialStatus,
    /// Sort by fulfillment status.
    FulfillmentStatus,
    /// Sort by destination.
    Destination,
    /// Sort by ID.
    Id,
}

impl OrderSortKey {
    /// Parse a sort key from a URL parameter string.
    #[must_use]
    pub fn from_str_param(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "order_number" | "number" => Some(Self::OrderNumber),
            "total_price" | "total" => Some(Self::TotalPrice),
            "created_at" | "created" => Some(Self::CreatedAt),
            "processed_at" | "processed" => Some(Self::ProcessedAt),
            "updated_at" | "updated" => Some(Self::UpdatedAt),
            "customer_name" | "customer" => Some(Self::CustomerName),
            "financial_status" | "payment" => Some(Self::FinancialStatus),
            "fulfillment_status" | "fulfillment" => Some(Self::FulfillmentStatus),
            "destination" => Some(Self::Destination),
            "id" => Some(Self::Id),
            _ => None,
        }
    }

    /// Get the URL parameter string for this sort key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OrderNumber => "order_number",
            Self::TotalPrice => "total_price",
            Self::CreatedAt => "created_at",
            Self::ProcessedAt => "processed_at",
            Self::UpdatedAt => "updated_at",
            Self::CustomerName => "customer_name",
            Self::FinancialStatus => "financial_status",
            Self::FulfillmentStatus => "fulfillment_status",
            Self::Destination => "destination",
            Self::Id => "id",
        }
    }
}

// =============================================================================
// Pagination Types
// =============================================================================

/// Paginated list of orders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderConnection {
    /// Orders in this page.
    pub orders: Vec<Order>,
    /// Pagination info.
    pub page_info: PageInfo,
}

/// Paginated list of orders (extended for list view).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderListConnection {
    /// Orders in this page.
    pub orders: Vec<OrderListItem>,
    /// Pagination info.
    pub page_info: PageInfo,
}

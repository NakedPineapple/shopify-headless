//! Domain types for Shopify Admin API.
//!
//! These types provide a clean, ergonomic API separate from the raw
//! `graphql_client` generated types.

use serde::{Deserialize, Serialize};

// =============================================================================
// Money Types
// =============================================================================

/// Monetary amount with currency code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    /// Decimal amount as string (preserves precision).
    pub amount: String,
    /// ISO 4217 currency code.
    pub currency_code: String,
}

// =============================================================================
// Image Types
// =============================================================================

/// Product or media image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    /// Shopify image ID.
    pub id: Option<String>,
    /// Image URL.
    pub url: String,
    /// Alt text for accessibility.
    pub alt_text: Option<String>,
    /// Image width in pixels.
    pub width: Option<i64>,
    /// Image height in pixels.
    pub height: Option<i64>,
}

// =============================================================================
// Address Types
// =============================================================================

/// Mailing address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    /// Address ID (for mutations).
    pub id: Option<String>,
    /// First line of the address.
    pub address1: Option<String>,
    /// Second line of the address.
    pub address2: Option<String>,
    /// City.
    pub city: Option<String>,
    /// Province or state code.
    pub province_code: Option<String>,
    /// Country code (ISO 3166-1 alpha-2).
    pub country_code: Option<String>,
    /// Postal/ZIP code.
    pub zip: Option<String>,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Company name.
    pub company: Option<String>,
    /// Phone number.
    pub phone: Option<String>,
}

/// Input for creating/updating a mailing address.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddressInput {
    /// First line of the address.
    pub address1: Option<String>,
    /// Second line of the address.
    pub address2: Option<String>,
    /// City.
    pub city: Option<String>,
    /// Province or state code.
    pub province_code: Option<String>,
    /// Country code (ISO 3166-1 alpha-2).
    pub country_code: Option<String>,
    /// Postal/ZIP code.
    pub zip: Option<String>,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Company name.
    pub company: Option<String>,
    /// Phone number.
    pub phone: Option<String>,
}

// =============================================================================
// Metafield Types
// =============================================================================

/// A metafield for storing custom data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metafield {
    /// Metafield ID.
    pub id: Option<String>,
    /// Namespace for grouping metafields.
    pub namespace: String,
    /// Key within the namespace.
    pub key: String,
    /// The metafield value.
    pub value: String,
}

/// Input for creating/updating metafields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetafieldInput {
    /// Namespace for the metafield.
    pub namespace: String,
    /// Key within the namespace.
    pub key: String,
    /// The value to store.
    pub value: String,
    /// The metafield type (e.g., `single_line_text_field`, `number_integer`).
    pub type_: String,
}

/// Parameters for updating a customer.
#[derive(Debug, Clone, Default)]
pub struct CustomerUpdateParams {
    /// Email address.
    pub email: Option<String>,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Phone number.
    pub phone: Option<String>,
    /// Note about the customer.
    pub note: Option<String>,
    /// Tags to set on the customer.
    pub tags: Option<Vec<String>>,
}

/// Override settings for customer merge operation.
///
/// Each field indicates whether to take the value from the source customer
/// (being merged) instead of the target customer (that remains).
// Allow: Each boolean represents an independent merge override choice from
// the Shopify API with no logical grouping into enums or state machines.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default)]
pub struct CustomerMergeOverrides {
    /// Take first name from the source customer.
    pub first_name: bool,
    /// Take last name from the source customer.
    pub last_name: bool,
    /// Take email from the source customer.
    pub email: bool,
    /// Take phone from the source customer.
    pub phone: bool,
    /// Take default address from the source customer.
    pub default_address: bool,
}

/// Identifier for a metafield (used in delete operations).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetafieldIdentifier {
    /// The owner resource ID.
    pub owner_id: String,
    /// Namespace of the metafield.
    pub namespace: String,
    /// Key of the metafield.
    pub key: String,
}

// =============================================================================
// Product Types
// =============================================================================

/// Product status in the admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProductStatus {
    /// Product is visible on the storefront.
    Active,
    /// Product is not visible (work in progress).
    Draft,
    /// Product is hidden/archived.
    Archived,
    /// Product is unlisted (not shown in search/collections but accessible via URL).
    Unlisted,
}

/// A product variant with admin-specific fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminProductVariant {
    /// Variant ID.
    pub id: String,
    /// Variant title (combination of option values).
    pub title: String,
    /// SKU code.
    pub sku: Option<String>,
    /// Barcode.
    pub barcode: Option<String>,
    /// Current price.
    pub price: Money,
    /// Compare-at price (original price if on sale).
    pub compare_at_price: Option<Money>,
    /// Inventory quantity (across all locations).
    pub inventory_quantity: i64,
    /// Inventory item ID (for inventory operations).
    pub inventory_item_id: String,
    /// Whether inventory is tracked.
    pub inventory_management: Option<String>,
    /// Weight value.
    pub weight: Option<f64>,
    /// Weight unit (KILOGRAMS, GRAMS, POUNDS, OUNCES).
    pub weight_unit: Option<String>,
    /// Whether requires shipping.
    pub requires_shipping: bool,
    /// Variant image.
    pub image: Option<Image>,
    /// Creation timestamp.
    pub created_at: Option<String>,
    /// Last update timestamp.
    pub updated_at: Option<String>,
}

/// A product in the admin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminProduct {
    /// Product ID.
    pub id: String,
    /// URL handle.
    pub handle: String,
    /// Product title.
    pub title: String,
    /// Plain text description.
    pub description: String,
    /// HTML description.
    pub description_html: String,
    /// Product status (Active, Draft, Archived).
    pub status: ProductStatus,
    /// Product type/category.
    #[serde(rename = "product_type")]
    pub kind: String,
    /// Vendor name.
    pub vendor: String,
    /// Product tags.
    pub tags: Vec<String>,
    /// Total inventory quantity across all variants.
    pub total_inventory: i64,
    /// Creation timestamp.
    pub created_at: Option<String>,
    /// Last update timestamp.
    pub updated_at: Option<String>,
    /// Featured image.
    pub featured_image: Option<Image>,
    /// All product images.
    pub images: Vec<Image>,
    /// Product variants.
    pub variants: Vec<AdminProductVariant>,
}

// =============================================================================
// Order Types
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

// =============================================================================
// Order Detail Types (for detail page)
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

// =============================================================================
// Customer Types
// =============================================================================

/// Customer account state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CustomerState {
    /// Customer has not yet accepted the invite.
    Disabled,
    /// Customer has accepted the invite.
    Enabled,
    /// Customer was invited but hasn't accepted.
    Invited,
    /// Customer account was declined.
    Declined,
}

/// Sort key for customer lists.
///
/// Some keys are supported natively by Shopify API, others require
/// client-side sorting in Rust after fetching data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CustomerSortKey {
    // === Shopify API supported ===
    /// Sort by creation date (Shopify native).
    CreatedAt,
    /// Sort by ID (Shopify native).
    Id,
    /// Sort by location (Shopify native).
    Location,
    /// Sort by name (Shopify native).
    #[default]
    Name,
    /// Sort by relevance for search queries (Shopify native).
    Relevance,
    /// Sort by last update date (Shopify native).
    UpdatedAt,

    // === Client-side sorting (Rust) ===
    /// Sort by total amount spent.
    AmountSpent,
    /// Sort by total orders count.
    OrdersCount,
}

impl CustomerSortKey {
    /// Parse a sort key from a URL parameter string.
    #[must_use]
    pub fn from_str_param(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "created_at" | "created" => Some(Self::CreatedAt),
            "id" => Some(Self::Id),
            "location" => Some(Self::Location),
            "name" => Some(Self::Name),
            "relevance" => Some(Self::Relevance),
            "updated_at" | "updated" => Some(Self::UpdatedAt),
            "amount_spent" | "spent" => Some(Self::AmountSpent),
            "orders_count" | "orders" => Some(Self::OrdersCount),
            _ => None,
        }
    }

    /// Get the URL parameter string for this sort key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CreatedAt => "created_at",
            Self::Id => "id",
            Self::Location => "location",
            Self::Name => "name",
            Self::Relevance => "relevance",
            Self::UpdatedAt => "updated_at",
            Self::AmountSpent => "amount_spent",
            Self::OrdersCount => "orders_count",
        }
    }

    /// Whether this sort key is supported natively by Shopify API.
    #[must_use]
    pub const fn is_shopify_native(self) -> bool {
        matches!(
            self,
            Self::CreatedAt
                | Self::Id
                | Self::Location
                | Self::Name
                | Self::Relevance
                | Self::UpdatedAt
        )
    }
}

/// Parameters for listing customers.
#[derive(Debug, Clone, Default)]
pub struct CustomerListParams {
    /// Maximum number of customers to return.
    pub first: Option<i64>,
    /// Cursor for pagination.
    pub after: Option<String>,
    /// Search/filter query string (Shopify query syntax).
    pub query: Option<String>,
    /// Sort key.
    pub sort_key: Option<CustomerSortKey>,
    /// Whether to reverse sort order.
    pub reverse: bool,
}

/// Marketing consent state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarketingState {
    /// Not subscribed to marketing.
    NotSubscribed,
    /// Pending confirmation.
    Pending,
    /// Subscribed to marketing.
    Subscribed,
    /// Unsubscribed from marketing.
    Unsubscribed,
    /// Data has been redacted.
    Redacted,
    /// Invalid state.
    Invalid,
}

/// Marketing consent information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketingConsent {
    /// Current marketing state.
    pub state: MarketingState,
    /// Opt-in level (e.g., `SINGLE_OPT_IN`, `CONFIRMED_OPT_IN`).
    pub opt_in_level: Option<String>,
    /// When consent was last updated.
    pub consent_updated_at: Option<String>,
}

/// A customer's recent order (for detail view).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerOrder {
    /// Order ID.
    pub id: String,
    /// Order name (e.g., "#1001").
    pub name: String,
    /// Creation timestamp.
    pub created_at: String,
    /// Financial status display string.
    pub financial_status: Option<String>,
    /// Fulfillment status display string.
    pub fulfillment_status: Option<String>,
    /// Total price.
    pub total_price: Money,
}

/// A customer in the admin.
// Allow: Shopify API Customer object has independent boolean properties
// (accepts_marketing, tax_exempt, can_delete, is_mergeable) that cannot be grouped.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    /// Customer ID.
    pub id: String,
    /// Email address.
    pub email: Option<String>,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Display name.
    pub display_name: String,
    /// Phone number.
    pub phone: Option<String>,
    /// Account state.
    pub state: CustomerState,
    /// Customer locale (language preference).
    pub locale: Option<String>,
    /// Whether email marketing is accepted (legacy field).
    pub accepts_marketing: bool,
    /// Marketing opt-in timestamp (legacy field).
    pub accepts_marketing_updated_at: Option<String>,
    /// Email marketing consent details.
    pub email_marketing_consent: Option<MarketingConsent>,
    /// SMS marketing consent details.
    pub sms_marketing_consent: Option<MarketingConsent>,
    /// Total orders count.
    pub orders_count: i64,
    /// Total amount spent.
    pub total_spent: Money,
    /// Human-readable lifetime duration (e.g., "2 years").
    pub lifetime_duration: Option<String>,
    /// Whether customer is tax exempt.
    pub tax_exempt: bool,
    /// List of tax exemption codes.
    pub tax_exemptions: Vec<String>,
    /// Customer note.
    pub note: Option<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Whether this customer can be deleted (no orders).
    pub can_delete: bool,
    /// Whether this customer can be merged with another.
    pub is_mergeable: bool,
    /// Default address.
    pub default_address: Option<Address>,
    /// All addresses.
    pub addresses: Vec<Address>,
    /// Recent orders (populated on detail view).
    pub recent_orders: Vec<CustomerOrder>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

// =============================================================================
// Collection Types
// =============================================================================

/// A rule that defines membership in a smart collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionRule {
    /// The attribute column to check (TAG, TITLE, VENDOR, `PRODUCT_TYPE`, etc.).
    pub column: String,
    /// The relation operator (EQUALS, `NOT_EQUALS`, CONTAINS, etc.).
    pub relation: String,
    /// The value to match against.
    pub condition: String,
}

/// A set of rules that define a smart collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionRuleSet {
    /// If true, products matching ANY rule are included (OR logic).
    /// If false, products must match ALL rules (AND logic).
    pub applied_disjunctively: bool,
    /// The individual rules in this set.
    pub rules: Vec<CollectionRule>,
}

/// SEO metadata for a collection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionSeo {
    /// SEO title (shown in search results).
    pub title: Option<String>,
    /// SEO meta description.
    pub description: Option<String>,
}

/// A sales channel/publication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Publication {
    /// Publication ID.
    pub id: String,
    /// Publication name (e.g., "Online Store", "TikTok").
    pub name: String,
}

/// Publication status for a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePublication {
    /// The publication/sales channel.
    pub publication: Publication,
    /// Whether the resource is published on this channel.
    pub is_published: bool,
}

/// A product collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    /// Collection ID.
    pub id: String,
    /// Collection title.
    pub title: String,
    /// URL handle.
    pub handle: String,
    /// Plain text description.
    pub description: String,
    /// HTML description.
    pub description_html: Option<String>,
    /// Number of products in the collection.
    pub products_count: i64,
    /// Collection image.
    pub image: Option<Image>,
    /// Last update timestamp.
    pub updated_at: Option<String>,
    /// Rule set for smart collections (None for manual collections).
    pub rule_set: Option<CollectionRuleSet>,
    /// Sort order for products in the collection.
    pub sort_order: Option<String>,
    /// SEO metadata.
    pub seo: Option<CollectionSeo>,
    /// Publication status on each sales channel.
    pub publications: Vec<ResourcePublication>,
}

/// A product within a collection (simplified view).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionProduct {
    /// Product ID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// URL handle.
    pub handle: String,
    /// Product status (ACTIVE, DRAFT, ARCHIVED).
    pub status: String,
    /// Featured image URL.
    pub image_url: Option<String>,
    /// Total inventory quantity.
    pub total_inventory: i64,
    /// Minimum variant price.
    pub price: String,
    /// Currency code.
    pub currency_code: String,
}

/// A collection with its products.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionWithProducts {
    /// The collection.
    pub collection: Collection,
    /// Products in this collection.
    pub products: Vec<CollectionProduct>,
    /// Whether there are more products to load.
    pub has_next_page: bool,
    /// Cursor for loading more products.
    pub end_cursor: Option<String>,
}

/// Paginated list of collections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionConnection {
    /// Collections in this page.
    pub collections: Vec<Collection>,
    /// Pagination info.
    pub page_info: PageInfo,
}

// =============================================================================
// Location Types
// =============================================================================

/// A physical location for inventory storage and fulfillment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    /// Location ID.
    pub id: String,
    /// Location name.
    pub name: String,
    /// Whether the location is active.
    pub is_active: bool,
    /// Whether this location fulfills online orders.
    pub fulfills_online_orders: bool,
    /// Location address.
    pub address: Option<LocationAddress>,
}

/// Simplified address for a location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationAddress {
    /// Street address.
    pub address1: Option<String>,
    /// City.
    pub city: Option<String>,
    /// Province/state code.
    pub province_code: Option<String>,
    /// Country code.
    pub country_code: Option<String>,
    /// Postal/ZIP code.
    pub zip: Option<String>,
}

/// Paginated list of locations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationConnection {
    /// Locations in this page.
    pub locations: Vec<Location>,
    /// Pagination info.
    pub page_info: PageInfo,
}

// =============================================================================
// Inventory Types
// =============================================================================

/// Inventory level at a specific location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryLevel {
    /// Inventory item ID.
    pub inventory_item_id: String,
    /// Location ID.
    pub location_id: String,
    /// Location name.
    pub location_name: Option<String>,
    /// Quantity available.
    pub available: i64,
    /// Quantity on hand.
    pub on_hand: i64,
    /// Quantity incoming.
    pub incoming: i64,
    /// Last update timestamp.
    pub updated_at: Option<String>,
}

/// Result of an inventory adjustment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryAdjustmentResult {
    /// The affected inventory level.
    pub inventory_level: InventoryLevel,
}

/// An inventory item with full details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItem {
    /// Inventory item ID.
    pub id: String,
    /// SKU code.
    pub sku: Option<String>,
    /// Whether inventory is tracked.
    pub tracked: bool,
    /// Whether the item requires shipping.
    pub requires_shipping: bool,
    /// Unit cost.
    pub unit_cost: Option<Money>,
    /// Harmonized System (HS) code for customs.
    pub harmonized_system_code: Option<String>,
    /// Country of origin (ISO 3166-1 alpha-2).
    pub country_code_of_origin: Option<String>,
    /// Province of origin.
    pub province_code_of_origin: Option<String>,
    /// Inventory levels at different locations.
    pub inventory_levels: Vec<InventoryLevel>,
    /// Associated variant.
    pub variant: Option<InventoryItemVariant>,
}

/// Variant info associated with an inventory item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItemVariant {
    /// Variant ID.
    pub id: String,
    /// Variant title.
    pub title: String,
    /// Display name.
    pub display_name: Option<String>,
    /// Price.
    pub price: Option<String>,
    /// Variant image.
    pub image: Option<Image>,
    /// Associated product.
    pub product: Option<InventoryItemProduct>,
}

/// Product info associated with an inventory item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItemProduct {
    /// Product ID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// URL handle.
    pub handle: String,
    /// Product status.
    pub status: ProductStatus,
    /// Featured image.
    pub featured_image: Option<Image>,
}

/// Paginated list of inventory items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItemConnection {
    /// Inventory items in this page.
    pub items: Vec<InventoryItem>,
    /// Pagination info.
    pub page_info: PageInfo,
}

/// Inventory adjustment reason codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InventoryAdjustmentReason {
    /// Correction to inventory count.
    Correction,
    /// Cycle count verification.
    CycleCountAvailable,
    /// Items damaged.
    Damaged,
    /// Items received from supplier.
    Received,
    /// Items restocked (returned to inventory).
    Restock,
    /// Inventory shrinkage (loss/theft).
    Shrinkage,
    /// Other reason.
    Other,
}

impl std::fmt::Display for InventoryAdjustmentReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Correction => write!(f, "Correction"),
            Self::CycleCountAvailable => write!(f, "Cycle Count"),
            Self::Damaged => write!(f, "Damaged"),
            Self::Received => write!(f, "Received"),
            Self::Restock => write!(f, "Restock"),
            Self::Shrinkage => write!(f, "Shrinkage"),
            Self::Other => write!(f, "Other"),
        }
    }
}

/// Input for updating an inventory item.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InventoryItemUpdateInput {
    /// New SKU.
    pub sku: Option<String>,
    /// Whether to track inventory.
    pub tracked: Option<bool>,
    /// Whether item requires shipping.
    pub requires_shipping: Option<bool>,
    /// Unit cost amount.
    pub cost: Option<String>,
    /// Harmonized System code.
    pub harmonized_system_code: Option<String>,
    /// Country of origin.
    pub country_code_of_origin: Option<String>,
    /// Province of origin.
    pub province_code_of_origin: Option<String>,
}

// =============================================================================
// Pagination Types
// =============================================================================

/// Pagination information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    /// Whether there are more items after this page.
    pub has_next_page: bool,
    /// Whether there are items before this page.
    pub has_previous_page: bool,
    /// Cursor for the first item.
    pub start_cursor: Option<String>,
    /// Cursor for the last item.
    pub end_cursor: Option<String>,
}

/// Paginated list of products.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminProductConnection {
    /// Products in this page.
    pub products: Vec<AdminProduct>,
    /// Pagination info.
    pub page_info: PageInfo,
}

/// Paginated list of orders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderConnection {
    /// Orders in this page.
    pub orders: Vec<Order>,
    /// Pagination info.
    pub page_info: PageInfo,
}

/// Paginated list of customers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerConnection {
    /// Customers in this page.
    pub customers: Vec<Customer>,
    /// Pagination info.
    pub page_info: PageInfo,
}

/// Paginated list of inventory levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryLevelConnection {
    /// Inventory levels in this page.
    pub inventory_levels: Vec<InventoryLevel>,
    /// Pagination info.
    pub page_info: PageInfo,
}

// =============================================================================
// Sort Keys
// =============================================================================

/// Sort keys for product queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProductSortKey {
    /// Sort by title.
    Title,
    /// Sort by product type.
    ProductType,
    /// Sort by vendor.
    Vendor,
    /// Sort by inventory total.
    InventoryTotal,
    /// Sort by last update.
    UpdatedAt,
    /// Sort by creation date.
    CreatedAt,
    /// Sort by ID.
    Id,
}

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

/// Risk assessment for an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRisk {
    /// Risk level.
    pub level: OrderRiskLevel,
    /// Risk message/reason.
    pub message: Option<String>,
}

/// Shipping line information for an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderShippingLine {
    /// Shipping method title.
    pub title: String,
    /// Delivery category.
    pub delivery_category: Option<DeliveryCategory>,
}

/// Channel information for an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderChannelInfo {
    /// Channel name (e.g., "Online Store", "POS", "Shop").
    pub channel_name: Option<String>,
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

/// Paginated list of orders (extended for list view).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderListConnection {
    /// Orders in this page.
    pub orders: Vec<OrderListItem>,
    /// Pagination info.
    pub page_info: PageInfo,
}

// =============================================================================
// Gift Card Types
// =============================================================================

/// A gift card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiftCard {
    /// Gift card ID.
    pub id: String,
    /// Last 4 characters of the code.
    pub last_characters: String,
    /// Current balance.
    pub balance: Money,
    /// Initial value.
    pub initial_value: Money,
    /// Expiration date.
    pub expires_on: Option<String>,
    /// Whether the gift card is enabled.
    pub enabled: bool,
    /// Creation timestamp.
    pub created_at: String,
    /// Associated customer email.
    pub customer_email: Option<String>,
    /// Associated customer name.
    pub customer_name: Option<String>,
    /// Note.
    pub note: Option<String>,
}

/// Paginated list of gift cards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiftCardConnection {
    /// Gift cards in this page.
    pub gift_cards: Vec<GiftCard>,
    /// Pagination info.
    pub page_info: PageInfo,
}

// =============================================================================
// Discount Types
// =============================================================================

/// Discount status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscountStatus {
    /// Discount is active.
    Active,
    /// Discount is expired.
    Expired,
    /// Discount is scheduled.
    Scheduled,
}

impl std::fmt::Display for DiscountStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "Active"),
            Self::Expired => write!(f, "Expired"),
            Self::Scheduled => write!(f, "Scheduled"),
        }
    }
}

/// Discount method (how the discount is applied).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscountMethod {
    /// Discount code that customers enter at checkout.
    Code,
    /// Automatic discount applied automatically.
    Automatic,
}

impl std::fmt::Display for DiscountMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Code => write!(f, "Code"),
            Self::Automatic => write!(f, "Automatic"),
        }
    }
}

/// Discount type (what kind of discount).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscountType {
    /// Basic amount off (percentage or fixed).
    Basic,
    /// Buy X Get Y discount.
    BuyXGetY,
    /// Free shipping discount.
    FreeShipping,
}

impl std::fmt::Display for DiscountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Basic => write!(f, "Amount Off"),
            Self::BuyXGetY => write!(f, "Buy X Get Y"),
            Self::FreeShipping => write!(f, "Free Shipping"),
        }
    }
}

/// Sort keys for discount queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscountSortKey {
    /// Sort by title.
    Title,
    /// Sort by creation date.
    #[default]
    CreatedAt,
    /// Sort by last update.
    UpdatedAt,
    /// Sort by start date.
    StartsAt,
    /// Sort by end date.
    EndsAt,
    /// Sort by ID.
    Id,
}

impl DiscountSortKey {
    /// Parse a sort key from a URL parameter string.
    #[must_use]
    pub fn from_str_param(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "title" => Some(Self::Title),
            "created_at" | "created" => Some(Self::CreatedAt),
            "updated_at" | "updated" => Some(Self::UpdatedAt),
            "starts_at" | "start" => Some(Self::StartsAt),
            "ends_at" | "end" => Some(Self::EndsAt),
            "id" => Some(Self::Id),
            _ => None,
        }
    }

    /// Get the URL parameter string for this sort key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::CreatedAt => "created_at",
            Self::UpdatedAt => "updated_at",
            Self::StartsAt => "starts_at",
            Self::EndsAt => "ends_at",
            Self::Id => "id",
        }
    }
}

/// Type of discount value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DiscountValue {
    /// Percentage discount.
    Percentage { percentage: f64 },
    /// Fixed amount discount.
    FixedAmount { amount: String, currency: String },
}

impl std::fmt::Display for DiscountValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Percentage { percentage } => {
                write!(f, "{}%", (percentage * 100.0).round())
            }
            Self::FixedAmount { amount, currency } => {
                write!(f, "${amount} {currency}")
            }
        }
    }
}

/// Discount combines-with settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscountCombinesWith {
    /// Can combine with order discounts.
    pub order_discounts: bool,
    /// Can combine with product discounts.
    pub product_discounts: bool,
    /// Can combine with shipping discounts.
    pub shipping_discounts: bool,
}

/// Minimum requirement for a discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscountMinimumRequirement {
    /// No minimum requirement.
    None,
    /// Minimum quantity of items.
    Quantity { quantity: String },
    /// Minimum subtotal amount.
    Subtotal { amount: String, currency: String },
}

impl std::fmt::Display for DiscountMinimumRequirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "No minimum"),
            Self::Quantity { quantity } => write!(f, "Min {quantity} items"),
            Self::Subtotal { amount, currency } => write!(f, "Min ${amount} {currency}"),
        }
    }
}

/// Customer eligibility for a discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscountCustomerEligibility {
    /// All customers.
    All,
    /// Specific customer segments.
    Segments { segments: Vec<CustomerSegment> },
    /// Specific customers.
    Customers { customers: Vec<CustomerRef> },
}

/// Reference to a customer segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerSegment {
    /// Segment ID.
    pub id: String,
    /// Segment name.
    pub name: String,
}

/// Reference to a customer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerRef {
    /// Customer ID.
    pub id: String,
    /// Display name.
    pub display_name: String,
    /// Email.
    pub email: Option<String>,
}

/// Items that a discount applies to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscountItems {
    /// All items.
    All,
    /// Specific products.
    Products { products: Vec<ProductRef> },
    /// Specific collections.
    Collections { collections: Vec<CollectionRef> },
}

/// Reference to a product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductRef {
    /// Product ID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// Featured image URL.
    pub image_url: Option<String>,
}

/// Reference to a collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionRef {
    /// Collection ID.
    pub id: String,
    /// Collection title.
    pub title: String,
    /// Collection image URL.
    pub image_url: Option<String>,
}

/// Buy X Get Y requirement type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BxgyRequirementType {
    /// Quantity-based (buy X items).
    Quantity,
    /// Amount-based (spend $X).
    Amount,
}

/// What the customer must buy for a BXGY discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BxgyCustomerBuys {
    /// Requirement type.
    pub requirement_type: BxgyRequirementType,
    /// Quantity required (if quantity-based).
    pub quantity: Option<String>,
    /// Amount required (if amount-based).
    pub amount: Option<String>,
    /// Items that qualify.
    pub items: DiscountItems,
}

/// What the customer gets for a BXGY discount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BxgyCustomerGets {
    /// Quantity the customer gets.
    pub quantity: String,
    /// Discount on those items.
    pub discount_value: DiscountValue,
    /// Items they can get.
    pub items: DiscountItems,
}

/// Shipping destination selection for free shipping discounts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShippingDestination {
    /// All countries.
    AllCountries,
    /// Specific countries.
    Countries {
        /// Country codes (ISO 3166-1 alpha-2).
        countries: Vec<String>,
        /// Include rest of world.
        include_rest_of_world: bool,
    },
}

/// A discount code (redeemable code).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountRedeemCode {
    /// Code ID.
    pub id: String,
    /// The code string.
    pub code: String,
    /// Number of times used.
    pub usage_count: i64,
}

/// A discount code (legacy type for backwards compatibility).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountCode {
    /// Discount node ID.
    pub id: String,
    /// Discount title.
    pub title: String,
    /// The actual code customers enter.
    pub code: String,
    /// Discount status.
    pub status: DiscountStatus,
    /// Start date.
    pub starts_at: Option<String>,
    /// End date.
    pub ends_at: Option<String>,
    /// Usage limit.
    pub usage_limit: Option<i64>,
    /// Number of times used.
    pub usage_count: i64,
    /// Discount value.
    pub value: Option<DiscountValue>,
}

/// A discount list item (for list view with all types).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountListItem {
    /// Discount node ID.
    pub id: String,
    /// Discount title.
    pub title: String,
    /// Primary code (for code discounts).
    pub code: Option<String>,
    /// Total code count (for code discounts).
    pub code_count: i64,
    /// Discount method (Code or Automatic).
    pub method: DiscountMethod,
    /// Discount type (Basic, `BuyXGetY`, `FreeShipping`).
    pub discount_type: DiscountType,
    /// Discount status.
    pub status: DiscountStatus,
    /// Discount value (for basic discounts).
    pub value: Option<DiscountValue>,
    /// Start date.
    pub starts_at: Option<String>,
    /// End date.
    pub ends_at: Option<String>,
    /// Usage limit (for code discounts).
    pub usage_limit: Option<i64>,
    /// Number of times used.
    pub usage_count: i64,
    /// Once per customer.
    pub once_per_customer: bool,
    /// Combines with settings.
    pub combines_with: DiscountCombinesWith,
    /// Minimum requirement.
    pub minimum_requirement: DiscountMinimumRequirement,
}

/// Full discount details (for detail/edit pages).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountDetail {
    /// Discount node ID.
    pub id: String,
    /// Discount title.
    pub title: String,
    /// Discount method (Code or Automatic).
    pub method: DiscountMethod,
    /// Discount type (Basic, `BuyXGetY`, `FreeShipping`).
    pub discount_type: DiscountType,
    /// Discount status.
    pub status: DiscountStatus,
    /// Start date.
    pub starts_at: Option<String>,
    /// End date.
    pub ends_at: Option<String>,
    /// Usage limit (for code discounts).
    pub usage_limit: Option<i64>,
    /// Number of times used.
    pub usage_count: i64,
    /// Once per customer.
    pub once_per_customer: bool,
    /// Recurring cycle limit (for subscriptions).
    pub recurring_cycle_limit: Option<i64>,
    /// Uses per order limit (for BXGY).
    pub uses_per_order_limit: Option<i64>,
    /// Combines with settings.
    pub combines_with: DiscountCombinesWith,
    /// Minimum requirement.
    pub minimum_requirement: DiscountMinimumRequirement,
    /// Customer eligibility.
    pub customer_eligibility: DiscountCustomerEligibility,
    /// Discount codes (for code discounts).
    pub codes: Vec<DiscountRedeemCode>,
    /// Total code count.
    pub code_count: i64,
    /// Discount value (for basic discounts).
    pub value: Option<DiscountValue>,
    /// Items discount applies to (for basic discounts).
    pub applies_to: Option<DiscountItems>,
    /// Customer buys (for BXGY).
    pub customer_buys: Option<BxgyCustomerBuys>,
    /// Customer gets (for BXGY).
    pub customer_gets: Option<BxgyCustomerGets>,
    /// Shipping destination (for free shipping).
    pub destination: Option<ShippingDestination>,
    /// Maximum shipping price (for free shipping).
    pub max_shipping_price: Option<Money>,
}

/// Paginated list of discount codes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountCodeConnection {
    /// Discount codes in this page.
    pub discount_codes: Vec<DiscountCode>,
    /// Pagination info.
    pub page_info: PageInfo,
}

/// Paginated list of discount list items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountListConnection {
    /// Discounts in this page.
    pub discounts: Vec<DiscountListItem>,
    /// Pagination info.
    pub page_info: PageInfo,
}

// =============================================================================
// Fulfillment Order Types
// =============================================================================

/// A fulfillment order represents a group of items that can be fulfilled together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentOrder {
    /// Fulfillment order ID.
    pub id: String,
    /// Fulfillment order status.
    pub status: String,
    /// Location ID where items will be fulfilled from.
    pub location_id: Option<String>,
    /// Location name.
    pub location_name: Option<String>,
    /// Line items in this fulfillment order.
    pub line_items: Vec<FulfillmentOrderLineItem>,
}

/// A line item in a fulfillment order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentOrderLineItem {
    /// Line item ID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// Variant title.
    pub variant_title: Option<String>,
    /// SKU.
    pub sku: Option<String>,
    /// Total quantity.
    pub total_quantity: i64,
    /// Remaining quantity to fulfill.
    pub remaining_quantity: i64,
}

// =============================================================================
// Payout Types
// =============================================================================

/// Payout status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayoutStatus {
    /// Payout is scheduled.
    Scheduled,
    /// Payout is in transit.
    InTransit,
    /// Payout has been paid.
    Paid,
    /// Payout failed.
    Failed,
    /// Payout was canceled.
    Canceled,
}

impl std::fmt::Display for PayoutStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scheduled => write!(f, "Scheduled"),
            Self::InTransit => write!(f, "In Transit"),
            Self::Paid => write!(f, "Paid"),
            Self::Failed => write!(f, "Failed"),
            Self::Canceled => write!(f, "Canceled"),
        }
    }
}

/// A Shopify Payments payout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payout {
    /// Payout ID.
    pub id: String,
    /// Legacy resource ID.
    pub legacy_resource_id: Option<String>,
    /// Payout status.
    pub status: PayoutStatus,
    /// Net amount (the payout amount).
    pub net: Money,
    /// When the payout was issued.
    pub issued_at: Option<String>,
}

/// Connection type for paginated payouts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutConnection {
    /// List of payouts.
    pub payouts: Vec<Payout>,
    /// Pagination info.
    pub page_info: PageInfo,
    /// Current account balance.
    pub balance: Option<Money>,
}

/// Staged upload target for file uploads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedUploadTarget {
    /// The URL to upload the file to.
    pub url: String,
    /// The resource URL after upload completes.
    pub resource_url: String,
    /// Form parameters to include with the upload.
    pub parameters: Vec<(String, String)>,
}

// =============================================================================
// Refund Types
// =============================================================================

/// Suggested refund calculation result.
#[derive(Debug, Clone)]
pub struct SuggestedRefundResult {
    /// Total refund amount.
    pub amount: String,
    /// Currency code.
    pub currency_code: String,
    /// Subtotal before tax.
    pub subtotal: String,
    /// Total tax amount.
    pub total_tax: String,
    /// Suggested line items to refund.
    pub line_items: Vec<SuggestedRefundLineItem>,
}

/// A line item in a suggested refund.
#[derive(Debug, Clone)]
pub struct SuggestedRefundLineItem {
    /// Line item ID.
    pub line_item_id: String,
    /// Line item title.
    pub title: String,
    /// Original quantity on the order.
    pub original_quantity: i64,
    /// Suggested quantity to refund.
    pub refund_quantity: i64,
}

// =============================================================================
// Refund Input Types
// =============================================================================

/// Input for creating a refund.
#[derive(Debug, Clone, Default)]
pub struct RefundCreateInput {
    /// Refund note/reason.
    pub note: Option<String>,
    /// Whether to notify the customer.
    pub notify: bool,
    /// Line items to refund.
    pub line_items: Vec<RefundLineItemInput>,
    /// Shipping refund amount (if any).
    pub shipping_amount: Option<String>,
    /// Whether to do a full shipping refund.
    pub full_shipping_refund: bool,
}

/// Input for a line item in a refund.
#[derive(Debug, Clone)]
pub struct RefundLineItemInput {
    /// Line item ID to refund.
    pub line_item_id: String,
    /// Quantity to refund.
    pub quantity: i64,
    /// How to handle restocking.
    pub restock_type: RefundRestockType,
    /// Location ID for restocking (required for RETURN type).
    pub location_id: Option<String>,
}

/// How to handle restocking for a refund.
#[derive(Debug, Clone, Copy, Default)]
pub enum RefundRestockType {
    /// Return items to inventory.
    Return,
    /// Cancel items (remove from order).
    Cancel,
    /// Don't restock.
    #[default]
    NoRestock,
}

// =============================================================================
// Fulfillment Hold Input Types
// =============================================================================

/// Reason for holding a fulfillment order.
#[derive(Debug, Clone, Copy)]
pub enum FulfillmentHoldReason {
    /// Waiting for payment.
    AwaitingPayment,
    /// High risk of fraud.
    HighRiskOfFraud,
    /// Incorrect address.
    IncorrectAddress,
    /// Inventory is out of stock.
    InventoryOutOfStock,
    /// Unknown delivery date.
    UnknownDeliveryDate,
    /// Awaiting return items.
    AwaitingReturnItems,
    /// Other reason.
    Other,
}

/// Input for holding a fulfillment order.
#[derive(Debug, Clone)]
pub struct FulfillmentHoldInput {
    /// Reason for the hold.
    pub reason: FulfillmentHoldReason,
    /// Additional notes about the hold.
    pub reason_notes: Option<String>,
    /// Whether to notify the merchant.
    pub notify_merchant: bool,
}

// =============================================================================
// Return Input Types
// =============================================================================

/// Input for creating a return.
#[derive(Debug, Clone)]
pub struct ReturnCreateInput {
    /// Line items to return.
    pub line_items: Vec<ReturnLineItemCreateInput>,
    /// When the return was requested.
    pub requested_at: Option<String>,
}

/// Input for a line item in a return.
#[derive(Debug, Clone)]
pub struct ReturnLineItemCreateInput {
    /// Fulfillment line item ID.
    pub fulfillment_line_item_id: String,
    /// Quantity to return.
    pub quantity: i64,
    /// Note about the return reason.
    pub return_reason_note: Option<String>,
}

// =============================================================================
// Order Edit Types
// =============================================================================

/// Staged status for a calculated shipping line during order editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CalculatedShippingLineStagedStatus {
    /// Shipping line existed before the edit.
    None,
    /// Shipping line was added during this edit.
    Added,
    /// Shipping line was removed during this edit.
    Removed,
}

impl std::fmt::Display for CalculatedShippingLineStagedStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "Unchanged"),
            Self::Added => write!(f, "Added"),
            Self::Removed => write!(f, "Removed"),
        }
    }
}

/// A shipping line in an order edit session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatedShippingLine {
    /// Shipping line ID (may be None for pre-existing lines).
    pub id: Option<String>,
    /// Shipping method title.
    pub title: String,
    /// Price of the shipping line.
    pub price: Money,
    /// Staged status indicating if this was added/removed during edit.
    pub staged_status: CalculatedShippingLineStagedStatus,
}

/// Allocated discount amount for a calculated line item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatedDiscountAllocation {
    /// The allocated discount amount.
    pub allocated_amount: Money,
    /// Description of the discount (if available).
    pub description: Option<String>,
}

/// A line item in an order edit session with editing context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatedLineItem {
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
    /// Quantity that can be edited (unfulfilled items).
    pub editable_quantity: i64,
    /// Editable quantity before any changes in this session.
    pub editable_quantity_before_changes: i64,
    /// Whether this item can be restocked.
    pub restockable: bool,
    /// Whether this item is being restocked.
    pub restocking: bool,
    /// Whether this item has a staged discount.
    pub has_staged_line_item_discount: bool,
    /// Original price per unit.
    pub original_unit_price: Money,
    /// Discounted price per unit.
    pub discounted_unit_price: Money,
    /// Editable subtotal (quantity × discounted price for editable items).
    pub editable_subtotal: Money,
    /// Line item image.
    pub image: Option<Image>,
    /// Variant ID.
    pub variant_id: Option<String>,
    /// Applied discount allocations.
    pub discount_allocations: Vec<CalculatedDiscountAllocation>,
}

/// Types of staged changes during an order edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OrderStagedChange {
    /// A variant was added to the order.
    AddVariant {
        /// Line item ID.
        line_item_id: String,
        /// Quantity added.
        quantity: i64,
    },
    /// A custom item was added to the order.
    AddCustomItem {
        /// Line item ID.
        line_item_id: String,
        /// Title of the custom item.
        title: String,
        /// Quantity added.
        quantity: i64,
    },
    /// A line item's quantity was increased.
    IncrementItem {
        /// Line item ID.
        line_item_id: String,
        /// Quantity delta.
        delta: i64,
    },
    /// A line item's quantity was decreased.
    DecrementItem {
        /// Line item ID.
        line_item_id: String,
        /// Quantity delta.
        delta: i64,
        /// Whether item is being restocked.
        restock: bool,
    },
    /// A discount was added to a line item.
    AddLineItemDiscount {
        /// Discount ID.
        discount_id: String,
        /// Line item ID.
        line_item_id: String,
        /// Discount description.
        description: Option<String>,
        /// Discount value (percentage or fixed amount).
        value: OrderEditDiscountValue,
    },
    /// A discount was removed.
    RemoveDiscount {
        /// Discount ID.
        discount_id: String,
    },
    /// A shipping line was added.
    AddShippingLine {
        /// Shipping line ID.
        shipping_line_id: String,
        /// Shipping method title.
        title: String,
        /// Shipping price.
        price: Money,
    },
    /// A shipping line was removed.
    RemoveShippingLine {
        /// Shipping line ID.
        shipping_line_id: String,
    },
}

/// Discount value for order edits (percentage or fixed amount).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderEditDiscountValue {
    /// Percentage discount (0.0 to 100.0).
    Percentage(f64),
    /// Fixed amount discount.
    FixedAmount(Money),
}

/// A calculated order representing an order edit session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatedOrder {
    /// Calculated order ID (used in subsequent mutations).
    pub id: String,
    /// The original order being edited.
    pub original_order_id: String,
    /// Original order name.
    pub original_order_name: String,
    /// Pre-existing line items with applied changes.
    pub line_items: Vec<CalculatedLineItem>,
    /// Newly added line items during this edit.
    pub added_line_items: Vec<CalculatedLineItem>,
    /// Shipping lines (existing and newly added).
    pub shipping_lines: Vec<CalculatedShippingLine>,
    /// Subtotal price after discounts (excluding shipping).
    pub subtotal_price: Money,
    /// Total price including shipping and taxes.
    pub total_price: Money,
    /// Amount the customer still owes (or will be refunded if negative).
    pub total_outstanding: Money,
    /// Total quantity of line items.
    pub subtotal_line_items_quantity: i64,
    /// Preview title for customer notification.
    pub notification_preview_title: Option<String>,
}

impl CalculatedOrder {
    /// Get all line items (existing + added).
    #[must_use]
    pub fn all_line_items(&self) -> Vec<&CalculatedLineItem> {
        self.line_items
            .iter()
            .chain(self.added_line_items.iter())
            .collect()
    }

    /// Check if there are any staged changes.
    #[must_use]
    pub fn has_changes(&self) -> bool {
        !self.added_line_items.is_empty()
            || self
                .line_items
                .iter()
                .any(|li| li.editable_quantity != li.editable_quantity_before_changes)
            || self
                .shipping_lines
                .iter()
                .any(|sl| sl.staged_status != CalculatedShippingLineStagedStatus::None)
    }

    /// Calculate the price difference from original order.
    #[must_use]
    pub const fn price_difference(&self) -> &Money {
        &self.total_outstanding
    }
}

// =============================================================================
// Order Edit Input Types
// =============================================================================

/// Input for applying a discount during order editing.
#[derive(Debug, Clone)]
pub struct OrderEditAppliedDiscountInput {
    /// Description of the discount.
    pub description: Option<String>,
    /// Fixed amount discount (mutually exclusive with `percent_value`).
    pub fixed_value: Option<Money>,
    /// Percentage discount (0.0 to 100.0, mutually exclusive with `fixed_value`).
    pub percent_value: Option<f64>,
}

impl OrderEditAppliedDiscountInput {
    /// Create a percentage discount.
    #[must_use]
    pub const fn percentage(percent: f64, description: Option<String>) -> Self {
        Self {
            description,
            fixed_value: None,
            percent_value: Some(percent),
        }
    }

    /// Create a fixed amount discount.
    #[must_use]
    pub const fn fixed_amount(amount: Money, description: Option<String>) -> Self {
        Self {
            description,
            fixed_value: Some(amount),
            percent_value: None,
        }
    }
}

/// Input for adding a shipping line during order editing.
#[derive(Debug, Clone)]
pub struct OrderEditAddShippingLineInput {
    /// Shipping method title.
    pub title: String,
    /// Shipping price.
    pub price: Money,
}

/// Input for updating a shipping line during order editing.
#[derive(Debug, Clone, Default)]
pub struct OrderEditUpdateShippingLineInput {
    /// New title (optional).
    pub title: Option<String>,
    /// New price (optional).
    pub price: Option<Money>,
}

/// Input for committing an order edit.
#[derive(Debug, Clone, Default)]
pub struct OrderEditCommitInput {
    /// Whether to notify the customer about the changes.
    pub notify_customer: bool,
    /// Internal staff note about the edit.
    pub staff_note: Option<String>,
}

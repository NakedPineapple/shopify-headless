//! Refund, fulfillment hold, and return input types for Shopify Admin API.

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

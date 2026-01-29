//! Order edit domain types for Shopify Admin API.

use serde::{Deserialize, Serialize};

use super::common::{Image, Money};

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

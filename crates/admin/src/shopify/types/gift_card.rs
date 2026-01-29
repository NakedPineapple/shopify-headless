//! Gift card domain types for Shopify Admin API.

use serde::{Deserialize, Serialize};

use super::common::{Money, PageInfo};

// =============================================================================
// Gift Card Types
// =============================================================================

/// Sort keys for gift card queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GiftCardSortKey {
    /// Sort by amount spent from the card.
    AmountSpent,
    /// Sort by current balance.
    Balance,
    /// Sort by gift card code.
    Code,
    /// Sort by creation date.
    #[default]
    CreatedAt,
    /// Sort by customer name.
    CustomerName,
    /// Sort by deactivation date.
    DisabledAt,
    /// Sort by expiration date.
    ExpiresOn,
    /// Sort by ID.
    Id,
    /// Sort by initial value.
    InitialValue,
    /// Sort by last update.
    UpdatedAt,
}

impl std::fmt::Display for GiftCardSortKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AmountSpent => write!(f, "Amount Spent"),
            Self::Balance => write!(f, "Balance"),
            Self::Code => write!(f, "Code"),
            Self::CreatedAt => write!(f, "Created"),
            Self::CustomerName => write!(f, "Customer"),
            Self::DisabledAt => write!(f, "Disabled"),
            Self::ExpiresOn => write!(f, "Expires"),
            Self::Id => write!(f, "ID"),
            Self::InitialValue => write!(f, "Initial Value"),
            Self::UpdatedAt => write!(f, "Updated"),
        }
    }
}

/// A gift card (list view).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiftCard {
    /// Gift card ID.
    pub id: String,
    /// Last 4 characters of the code.
    pub last_characters: String,
    /// Masked code (all but last 4 chars hidden).
    pub masked_code: Option<String>,
    /// Current balance.
    pub balance: Money,
    /// Initial value.
    pub initial_value: Money,
    /// Expiration date (YYYY-MM-DD).
    pub expires_on: Option<String>,
    /// Whether the gift card is enabled.
    pub enabled: bool,
    /// When the gift card was deactivated.
    pub deactivated_at: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: Option<String>,
    /// Associated customer ID.
    pub customer_id: Option<String>,
    /// Associated customer email.
    pub customer_email: Option<String>,
    /// Associated customer name.
    pub customer_name: Option<String>,
    /// Internal note.
    pub note: Option<String>,
    /// Associated order ID.
    pub order_id: Option<String>,
    /// Associated order name.
    pub order_name: Option<String>,
}

/// Paginated list of gift cards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiftCardConnection {
    /// Gift cards in this page.
    pub gift_cards: Vec<GiftCard>,
    /// Pagination info.
    pub page_info: PageInfo,
    /// Total count (if requested).
    pub total_count: Option<i64>,
}

/// Gift card transaction (credit or debit).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiftCardTransaction {
    /// Transaction ID.
    pub id: String,
    /// Transaction amount.
    pub amount: Money,
    /// When the transaction was processed.
    pub processed_at: String,
    /// Note attached to the transaction.
    pub note: Option<String>,
    /// Whether this is a credit (true) or debit (false).
    pub is_credit: bool,
}

/// Gift card recipient details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiftCardRecipient {
    /// Recipient customer ID.
    pub recipient_id: Option<String>,
    /// Recipient display name.
    pub recipient_name: Option<String>,
    /// Recipient email.
    pub recipient_email: Option<String>,
    /// Preferred name for the recipient.
    pub preferred_name: Option<String>,
    /// Gift message.
    pub message: Option<String>,
    /// When to send notification.
    pub send_notification_at: Option<String>,
}

/// A gift card with full details (detail view).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiftCardDetail {
    /// Gift card ID.
    pub id: String,
    /// Last 4 characters of the code.
    pub last_characters: String,
    /// Masked code (all but last 4 chars hidden).
    pub masked_code: String,
    /// Current balance.
    pub balance: Money,
    /// Initial value.
    pub initial_value: Money,
    /// Expiration date (YYYY-MM-DD).
    pub expires_on: Option<String>,
    /// Whether the gift card is enabled.
    pub enabled: bool,
    /// When the gift card was deactivated.
    pub deactivated_at: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
    /// Internal note.
    pub note: Option<String>,
    /// Template suffix for rendering.
    pub template_suffix: Option<String>,
    /// Associated customer ID.
    pub customer_id: Option<String>,
    /// Associated customer name.
    pub customer_name: Option<String>,
    /// Associated customer email.
    pub customer_email: Option<String>,
    /// Associated customer phone.
    pub customer_phone: Option<String>,
    /// Recipient details.
    pub recipient: Option<GiftCardRecipient>,
    /// Associated order ID.
    pub order_id: Option<String>,
    /// Associated order name.
    pub order_name: Option<String>,
    /// Associated order creation date.
    pub order_created_at: Option<String>,
    /// Transaction history.
    pub transactions: Vec<GiftCardTransaction>,
}

/// Gift card configuration (shop limits).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GiftCardConfiguration {
    /// Maximum value for manually issued gift cards.
    pub issue_limit: Option<Money>,
    /// Maximum value for purchased gift cards.
    pub purchase_limit: Option<Money>,
}

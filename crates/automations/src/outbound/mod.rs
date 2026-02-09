//! Outbound email event tracking.
//!
//! Customer-facing emails are triggered via Klaviyo events (the poller detects
//! Shopify order/fulfillment changes and fires Klaviyo events). Internal emails
//! (low stock alerts) are sent directly via SMTP.
//!
//! The `outbound_email_queue` table is retained for deduplication tracking.

pub mod poller;

/// The type of transactional email being sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailType {
    OrderConfirmation,
    ShippingUpdate,
    DeliveryNotification,
    SubscriptionRenewalReminder,
    SubscriptionWinBack,
}

impl EmailType {
    /// String representation stored in the database `email_type` column.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OrderConfirmation => "order_confirmation",
            Self::ShippingUpdate => "shipping_update",
            Self::DeliveryNotification => "delivery_notification",
            Self::SubscriptionRenewalReminder => "subscription_renewal_reminder",
            Self::SubscriptionWinBack => "subscription_winback",
        }
    }
}

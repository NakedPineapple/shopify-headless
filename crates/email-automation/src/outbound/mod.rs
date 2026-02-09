//! Transactional outbound email system.
//!
//! Replaces Shopify's built-in notification emails with branded Naked Pineapple
//! templates. Emails are queued in `outbound_email_queue` and sent via the
//! Microsoft 365 Graph API `sendMail` endpoint.
//!
//! # Workflow
//!
//! 1. **Poller**: Queries Shopify for recent orders/fulfillments
//! 2. **Enqueue**: Renders templates and inserts into the queue
//! 3. **Sender**: Processes the queue every 30s, sends via M365

pub mod poller;
pub mod sender;
mod templates;

use askama::Template;
use tracing::{debug, instrument};

use crate::db::outbound_queue::{self, EnqueueParams};

/// The type of transactional email being sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailType {
    OrderConfirmation,
    ShippingUpdate,
    DeliveryNotification,
    ReviewRequest,
}

impl EmailType {
    /// String representation stored in the database `email_type` column.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OrderConfirmation => "order_confirmation",
            Self::ShippingUpdate => "shipping_update",
            Self::DeliveryNotification => "delivery_notification",
            Self::ReviewRequest => "review_request",
        }
    }
}

/// Data needed to render and queue an order confirmation email.
pub struct OrderConfirmationData {
    pub customer_name: String,
    pub order_name: String,
    pub order_date: String,
    pub line_items: Vec<LineItemData>,
    pub subtotal: String,
    pub shipping: String,
    pub tax: String,
    pub total: String,
    pub shipping_address: Option<AddressData>,
}

/// A single line item in an order.
pub struct LineItemData {
    pub title: String,
    pub variant: Option<String>,
    pub quantity: i64,
    pub price: String,
}

/// A shipping address.
pub struct AddressData {
    pub name: String,
    pub address1: String,
    pub address2: Option<String>,
    pub city: String,
    pub province: String,
    pub zip: String,
    pub country: String,
}

/// Data needed to render and queue a shipping update email.
pub struct ShippingUpdateData {
    pub customer_name: String,
    pub order_name: String,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub tracking_url: Option<String>,
    pub items: Vec<String>,
}

/// Data needed to render and queue a delivery notification email.
pub struct DeliveryNotificationData {
    pub customer_name: String,
    pub order_name: String,
}

/// Data needed to render and queue a review request email.
pub struct ReviewRequestData {
    pub customer_name: String,
    pub product_names: Vec<String>,
    pub store_url: String,
}

/// Render and enqueue an order confirmation email.
#[instrument(skip(pool, data), fields(order = %data.order_name))]
pub async fn enqueue_order_confirmation(
    pool: &sqlx::PgPool,
    to_address: &str,
    to_name: Option<&str>,
    order_id: &str,
    data: &OrderConfirmationData,
) -> Result<i64, OutboundError> {
    let html = templates::OrderConfirmationHtml::from_data(data).render()?;
    let text = templates::OrderConfirmationText::from_data(data).render()?;
    let subject = format!("Order {} Confirmed — Naked Pineapple", data.order_name);

    debug!("queueing order confirmation email");

    let id = outbound_queue::enqueue(
        pool,
        &EnqueueParams {
            email_type: EmailType::OrderConfirmation.as_str(),
            to_address,
            to_name,
            subject: &subject,
            body_html: &html,
            body_text: &text,
            reference_id: Some(order_id),
            reference_type: Some("order"),
            scheduled_for: None,
        },
    )
    .await?;

    Ok(id)
}

/// Render and enqueue a shipping update email.
#[instrument(skip(pool, data), fields(order = %data.order_name))]
pub async fn enqueue_shipping_update(
    pool: &sqlx::PgPool,
    to_address: &str,
    to_name: Option<&str>,
    order_id: &str,
    data: &ShippingUpdateData,
) -> Result<i64, OutboundError> {
    let html = templates::ShippingUpdateHtml::from_data(data).render()?;
    let text = templates::ShippingUpdateText::from_data(data).render()?;
    let subject = format!("Your Order {} Has Shipped!", data.order_name);

    debug!("queueing shipping update email");

    let id = outbound_queue::enqueue(
        pool,
        &EnqueueParams {
            email_type: EmailType::ShippingUpdate.as_str(),
            to_address,
            to_name,
            subject: &subject,
            body_html: &html,
            body_text: &text,
            reference_id: Some(order_id),
            reference_type: Some("fulfillment"),
            scheduled_for: None,
        },
    )
    .await?;

    Ok(id)
}

/// Render and enqueue a delivery notification email.
#[instrument(skip(pool, data), fields(order = %data.order_name))]
pub async fn enqueue_delivery_notification(
    pool: &sqlx::PgPool,
    to_address: &str,
    to_name: Option<&str>,
    order_id: &str,
    data: &DeliveryNotificationData,
) -> Result<i64, OutboundError> {
    let html = templates::DeliveryNotificationHtml::from_data(data).render()?;
    let text = templates::DeliveryNotificationText::from_data(data).render()?;
    let subject = format!("Your Order {} Has Been Delivered!", data.order_name);

    debug!("queueing delivery notification email");

    let id = outbound_queue::enqueue(
        pool,
        &EnqueueParams {
            email_type: EmailType::DeliveryNotification.as_str(),
            to_address,
            to_name,
            subject: &subject,
            body_html: &html,
            body_text: &text,
            reference_id: Some(order_id),
            reference_type: Some("delivery"),
            scheduled_for: None,
        },
    )
    .await?;

    Ok(id)
}

/// Render and enqueue a review request email, scheduled for a future date.
#[instrument(skip(pool, data))]
pub async fn enqueue_review_request(
    pool: &sqlx::PgPool,
    to_address: &str,
    to_name: Option<&str>,
    order_id: &str,
    data: &ReviewRequestData,
    scheduled_for: chrono::DateTime<chrono::Utc>,
) -> Result<i64, OutboundError> {
    let html = templates::ReviewRequestHtml::from_data(data).render()?;
    let text = templates::ReviewRequestText::from_data(data).render()?;
    let subject = "How Are You Loving Your Naked Pineapple Products?".to_string();

    debug!("queueing review request email");

    let id = outbound_queue::enqueue(
        pool,
        &EnqueueParams {
            email_type: EmailType::ReviewRequest.as_str(),
            to_address,
            to_name,
            subject: &subject,
            body_html: &html,
            body_text: &text,
            reference_id: Some(order_id),
            reference_type: Some("review_request"),
            scheduled_for: Some(scheduled_for),
        },
    )
    .await?;

    Ok(id)
}

/// Errors from outbound email operations.
#[derive(Debug, thiserror::Error)]
pub enum OutboundError {
    /// Template rendering failed.
    #[error("template error: {0}")]
    Template(#[from] askama::Error),

    /// Database operation failed.
    #[error("database error: {0}")]
    Database(#[from] crate::db::RepositoryError),
}

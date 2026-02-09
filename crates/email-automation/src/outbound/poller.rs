//! Shopify order/fulfillment poller for triggering transactional emails.
//!
//! Periodically queries the Shopify Admin API for recent order events and
//! queues the appropriate transactional emails. Deduplicates against the
//! `outbound_email_queue` table to avoid sending the same email twice.

use chrono::Utc;
use sqlx::PgPool;
use tracing::{debug, error, info, instrument, warn};

use super::{
    AddressData, DeliveryNotificationData, EmailType, LineItemData, OrderConfirmationData,
    ReviewRequestData, ShippingUpdateData,
};
use crate::db::outbound_queue;
use crate::shopify::ShopifyClient;
use crate::shopify::fulfillments::{self, OrderDetail};

/// Poll Shopify for new orders and queue confirmation emails.
#[instrument(skip(pool, shopify))]
pub async fn poll_new_orders(pool: &PgPool, shopify: &ShopifyClient, poll_minutes: u64) {
    let orders = match fulfillments::fetch_recent_orders(shopify, poll_minutes).await {
        Ok(orders) => orders,
        Err(e) => {
            error!(error = %e, "failed to fetch recent orders from Shopify");
            return;
        }
    };

    debug!(count = orders.len(), "fetched recent orders");

    for order in &orders {
        if let Err(e) = maybe_queue_order_confirmation(pool, order).await {
            warn!(order = %order.name, error = %e, "failed to queue order confirmation");
        }
    }
}

/// Poll Shopify for recently shipped orders and queue shipping emails.
#[instrument(skip(pool, shopify))]
pub async fn poll_fulfillments(pool: &PgPool, shopify: &ShopifyClient, poll_minutes: u64) {
    let orders = match fulfillments::fetch_recently_fulfilled(shopify, poll_minutes).await {
        Ok(orders) => orders,
        Err(e) => {
            error!(error = %e, "failed to fetch recent fulfillments from Shopify");
            return;
        }
    };

    debug!(count = orders.len(), "fetched recently fulfilled orders");

    for order in &orders {
        if let Err(e) = maybe_queue_shipping_update(pool, order).await {
            warn!(order = %order.name, error = %e, "failed to queue shipping update");
        }
    }
}

/// Poll Shopify for recently delivered orders and queue delivery + review emails.
#[instrument(skip(pool, shopify))]
pub async fn poll_deliveries(
    pool: &PgPool,
    shopify: &ShopifyClient,
    poll_minutes: u64,
    review_delay_days: u64,
) {
    let orders = match fulfillments::fetch_recently_delivered(shopify, poll_minutes).await {
        Ok(orders) => orders,
        Err(e) => {
            error!(error = %e, "failed to fetch recently delivered orders from Shopify");
            return;
        }
    };

    debug!(count = orders.len(), "fetched recently delivered orders");

    for order in &orders {
        if let Err(e) = maybe_queue_delivery_notification(pool, order).await {
            warn!(order = %order.name, error = %e, "failed to queue delivery notification");
        }
        if let Err(e) = maybe_queue_review_request(pool, order, review_delay_days).await {
            warn!(order = %order.name, error = %e, "failed to queue review request");
        }
    }
}

async fn maybe_queue_order_confirmation(
    pool: &PgPool,
    order: &OrderDetail,
) -> Result<(), super::OutboundError> {
    let Some(email) = &order.email else {
        return Ok(());
    };
    let order_ref = &order.id;

    if outbound_queue::exists(pool, EmailType::OrderConfirmation.as_str(), order_ref).await? {
        return Ok(());
    }

    let customer_name = customer_name(order);
    let data = build_confirmation_data(order, &customer_name);
    let to_name = if customer_name.is_empty() {
        None
    } else {
        Some(customer_name.as_str())
    };

    let id = super::enqueue_order_confirmation(pool, email, to_name, order_ref, &data).await?;
    info!(email_id = id, order = %order.name, "queued order confirmation");
    Ok(())
}

async fn maybe_queue_shipping_update(
    pool: &PgPool,
    order: &OrderDetail,
) -> Result<(), super::OutboundError> {
    let Some(email) = &order.email else {
        return Ok(());
    };
    let order_ref = &order.id;

    if outbound_queue::exists(pool, EmailType::ShippingUpdate.as_str(), order_ref).await? {
        return Ok(());
    }

    let customer_name = customer_name(order);
    let data = build_shipping_data(order, &customer_name);
    let to_name = if customer_name.is_empty() {
        None
    } else {
        Some(customer_name.as_str())
    };

    let id = super::enqueue_shipping_update(pool, email, to_name, order_ref, &data).await?;
    info!(email_id = id, order = %order.name, "queued shipping update");
    Ok(())
}

async fn maybe_queue_delivery_notification(
    pool: &PgPool,
    order: &OrderDetail,
) -> Result<(), super::OutboundError> {
    let Some(email) = &order.email else {
        return Ok(());
    };
    let order_ref = &order.id;

    if outbound_queue::exists(pool, EmailType::DeliveryNotification.as_str(), order_ref).await? {
        return Ok(());
    }

    let customer_name = customer_name(order);
    let data = DeliveryNotificationData {
        customer_name: customer_name.clone(),
        order_name: order.name.clone(),
    };
    let to_name = if customer_name.is_empty() {
        None
    } else {
        Some(customer_name.as_str())
    };

    let id = super::enqueue_delivery_notification(pool, email, to_name, order_ref, &data).await?;
    info!(email_id = id, order = %order.name, "queued delivery notification");
    Ok(())
}

async fn maybe_queue_review_request(
    pool: &PgPool,
    order: &OrderDetail,
    delay_days: u64,
) -> Result<(), super::OutboundError> {
    let Some(email) = &order.email else {
        return Ok(());
    };
    let order_ref = &order.id;

    if outbound_queue::exists(pool, EmailType::ReviewRequest.as_str(), order_ref).await? {
        return Ok(());
    }

    let customer_name = customer_name(order);
    let product_names: Vec<String> = order.line_items.iter().map(|li| li.title.clone()).collect();
    let data = ReviewRequestData {
        customer_name: customer_name.clone(),
        product_names,
        store_url: "https://nakedpineapple.co".to_string(),
    };
    let to_name = if customer_name.is_empty() {
        None
    } else {
        Some(customer_name.as_str())
    };

    let scheduled_for = Utc::now() + chrono::Duration::days(i64::try_from(delay_days).unwrap_or(7));

    let id = super::enqueue_review_request(pool, email, to_name, order_ref, &data, scheduled_for)
        .await?;
    info!(
        email_id = id,
        order = %order.name,
        scheduled = %scheduled_for,
        "queued review request"
    );
    Ok(())
}

fn customer_name(order: &OrderDetail) -> String {
    match (&order.customer_first_name, &order.customer_last_name) {
        (Some(first), Some(last)) => format!("{first} {last}"),
        (Some(first), None) => first.clone(),
        (None, Some(last)) => last.clone(),
        (None, None) => String::new(),
    }
}

fn build_confirmation_data(order: &OrderDetail, customer_name: &str) -> OrderConfirmationData {
    let line_items = order
        .line_items
        .iter()
        .map(|li| LineItemData {
            title: li.title.clone(),
            variant: li.variant_title.clone(),
            quantity: li.quantity,
            price: format!("${}", li.price),
        })
        .collect();

    let shipping_address = order.shipping_address.as_ref().map(|addr| {
        let name = match (&addr.first_name, &addr.last_name) {
            (Some(f), Some(l)) => format!("{f} {l}"),
            (Some(f), None) => f.clone(),
            (None, Some(l)) => l.clone(),
            (None, None) => String::new(),
        };
        AddressData {
            name,
            address1: addr.address1.clone().unwrap_or_default(),
            address2: addr.address2.clone(),
            city: addr.city.clone().unwrap_or_default(),
            province: addr.province_code.clone().unwrap_or_default(),
            zip: addr.zip.clone().unwrap_or_default(),
            country: addr.country.clone().unwrap_or_default(),
        }
    });

    OrderConfirmationData {
        customer_name: customer_name.to_string(),
        order_name: order.name.clone(),
        order_date: order.created_at.clone(),
        line_items,
        subtotal: format!("${}", order.prices.subtotal),
        shipping: format!("${}", order.prices.shipping),
        tax: format!("${}", order.prices.tax),
        total: format!("${}", order.prices.total),
        shipping_address,
    }
}

fn build_shipping_data(order: &OrderDetail, customer_name: &str) -> ShippingUpdateData {
    let fulfillment = order.fulfillments.first();
    let items = order
        .line_items
        .iter()
        .map(|li| {
            li.variant_title.as_ref().map_or_else(
                || format!("{} x{}", li.title, li.quantity),
                |variant| format!("{} ({variant}) x{}", li.title, li.quantity),
            )
        })
        .collect();

    ShippingUpdateData {
        customer_name: customer_name.to_string(),
        order_name: order.name.clone(),
        carrier: fulfillment.and_then(|f| f.company.clone()),
        tracking_number: fulfillment.and_then(|f| f.number.clone()),
        tracking_url: fulfillment.and_then(|f| f.url.clone()),
        items,
    }
}

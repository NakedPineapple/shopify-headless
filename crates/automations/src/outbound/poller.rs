//! Shopify order/fulfillment poller for triggering Klaviyo transactional events.
//!
//! Periodically queries the Shopify Admin API for recent order events and
//! fires Klaviyo events to trigger email flows. Deduplicates against the
//! `outbound_email_queue` table to avoid firing the same event twice.

use sqlx::PgPool;
use tracing::{debug, error, info, instrument, warn};

use naked_pineapple_services::klaviyo::KlaviyoClient;
use naked_pineapple_services::klaviyo::events::{
    OrderConfirmedEventParams, OrderDeliveredEventParams, OrderLineItemEvent,
    OrderShippedEventParams, ShippingAddressEvent,
};

use super::EmailType;
use crate::db::outbound_queue;
use crate::shopify::ShopifyClient;
use crate::shopify::fulfillments::{self, OrderDetail};

/// Poll Shopify for new orders and fire "Order Confirmed" events.
#[instrument(skip(pool, shopify, klaviyo))]
pub async fn poll_new_orders(
    pool: &PgPool,
    shopify: &ShopifyClient,
    klaviyo: &KlaviyoClient,
    poll_minutes: u64,
) {
    let orders = match fulfillments::fetch_recent_orders(shopify, poll_minutes).await {
        Ok(orders) => orders,
        Err(e) => {
            error!(error = %e, "failed to fetch recent orders from Shopify");
            return;
        }
    };

    debug!(count = orders.len(), "fetched recent orders");

    for order in &orders {
        if let Err(e) = maybe_track_order_confirmed(pool, klaviyo, order).await {
            warn!(order = %order.name, error = %e, "failed to track order confirmed event");
        }
    }
}

/// Poll Shopify for recently shipped orders and fire "Order Shipped" events.
#[instrument(skip(pool, shopify, klaviyo))]
pub async fn poll_fulfillments(
    pool: &PgPool,
    shopify: &ShopifyClient,
    klaviyo: &KlaviyoClient,
    poll_minutes: u64,
) {
    let orders = match fulfillments::fetch_recently_fulfilled(shopify, poll_minutes).await {
        Ok(orders) => orders,
        Err(e) => {
            error!(error = %e, "failed to fetch recent fulfillments from Shopify");
            return;
        }
    };

    debug!(count = orders.len(), "fetched recently fulfilled orders");

    for order in &orders {
        if let Err(e) = maybe_track_order_shipped(pool, klaviyo, order).await {
            warn!(order = %order.name, error = %e, "failed to track order shipped event");
        }
    }
}

/// Poll Shopify for recently delivered orders and fire "Order Delivered" events.
///
/// The "Order Delivered" event triggers both a delivery notification and,
/// after a configurable delay in the Klaviyo flow, a review request email.
#[instrument(skip(pool, shopify, klaviyo))]
pub async fn poll_deliveries(
    pool: &PgPool,
    shopify: &ShopifyClient,
    klaviyo: &KlaviyoClient,
    poll_minutes: u64,
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
        if let Err(e) = maybe_track_order_delivered(pool, klaviyo, order).await {
            warn!(order = %order.name, error = %e, "failed to track order delivered event");
        }
    }
}

pub(crate) async fn maybe_track_order_confirmed(
    pool: &PgPool,
    klaviyo: &KlaviyoClient,
    order: &OrderDetail,
) -> Result<(), PollerError> {
    let Some(email) = &order.email else {
        return Ok(());
    };
    let order_ref = &order.id;

    if outbound_queue::exists(pool, EmailType::OrderConfirmation.as_str(), order_ref).await? {
        return Ok(());
    }

    let customer_name = customer_name(order);
    let line_items = build_line_items(order);
    let shipping_address = build_shipping_address(order);

    let params = OrderConfirmedEventParams {
        email,
        customer_name: &customer_name,
        order_name: &order.name,
        order_date: &order.created_at,
        line_items: &line_items,
        subtotal: &format!("${}", order.prices.subtotal),
        shipping: &format!("${}", order.prices.shipping),
        tax: &format!("${}", order.prices.tax),
        total: &format!("${}", order.prices.total),
        shipping_address: shipping_address.as_ref(),
    };

    klaviyo.track_order_confirmed_event(&params).await?;
    outbound_queue::record_tracked(
        pool,
        EmailType::OrderConfirmation.as_str(),
        order_ref,
        "order",
    )
    .await?;

    info!(order = %order.name, "tracked order confirmed event");
    Ok(())
}

pub(crate) async fn maybe_track_order_shipped(
    pool: &PgPool,
    klaviyo: &KlaviyoClient,
    order: &OrderDetail,
) -> Result<(), PollerError> {
    let Some(email) = &order.email else {
        return Ok(());
    };
    let order_ref = &order.id;

    if outbound_queue::exists(pool, EmailType::ShippingUpdate.as_str(), order_ref).await? {
        return Ok(());
    }

    let customer_name = customer_name(order);
    let fulfillment = order.fulfillments.first();
    let items: Vec<String> = order
        .line_items
        .iter()
        .map(|li| {
            li.variant_title.as_ref().map_or_else(
                || format!("{} x{}", li.title, li.quantity),
                |variant| format!("{} ({variant}) x{}", li.title, li.quantity),
            )
        })
        .collect();

    let params = OrderShippedEventParams {
        email,
        customer_name: &customer_name,
        order_name: &order.name,
        carrier: fulfillment.and_then(|f| f.company.as_deref()),
        tracking_number: fulfillment.and_then(|f| f.number.as_deref()),
        tracking_url: fulfillment.and_then(|f| f.url.as_deref()),
        items: &items,
    };

    klaviyo.track_order_shipped_event(&params).await?;
    outbound_queue::record_tracked(
        pool,
        EmailType::ShippingUpdate.as_str(),
        order_ref,
        "fulfillment",
    )
    .await?;

    info!(order = %order.name, "tracked order shipped event");
    Ok(())
}

pub(crate) async fn maybe_track_order_delivered(
    pool: &PgPool,
    klaviyo: &KlaviyoClient,
    order: &OrderDetail,
) -> Result<(), PollerError> {
    let Some(email) = &order.email else {
        return Ok(());
    };
    let order_ref = &order.id;

    if outbound_queue::exists(pool, EmailType::DeliveryNotification.as_str(), order_ref).await? {
        return Ok(());
    }

    let customer_name = customer_name(order);
    let product_names: Vec<String> = order.line_items.iter().map(|li| li.title.clone()).collect();

    let params = OrderDeliveredEventParams {
        email,
        customer_name: &customer_name,
        order_name: &order.name,
        product_names: &product_names,
        store_url: "https://nakedpineapple.co",
    };

    klaviyo.track_order_delivered_event(&params).await?;
    outbound_queue::record_tracked(
        pool,
        EmailType::DeliveryNotification.as_str(),
        order_ref,
        "delivery",
    )
    .await?;

    info!(order = %order.name, "tracked order delivered event");
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

fn build_line_items(order: &OrderDetail) -> Vec<OrderLineItemEvent> {
    order
        .line_items
        .iter()
        .map(|li| OrderLineItemEvent {
            title: li.title.clone(),
            variant: li.variant_title.clone(),
            quantity: li.quantity,
            price: format!("${}", li.price),
        })
        .collect()
}

fn build_shipping_address(order: &OrderDetail) -> Option<ShippingAddressEvent> {
    order.shipping_address.as_ref().map(|addr| {
        let name = match (&addr.first_name, &addr.last_name) {
            (Some(f), Some(l)) => format!("{f} {l}"),
            (Some(f), None) => f.clone(),
            (None, Some(l)) => l.clone(),
            (None, None) => String::new(),
        };
        ShippingAddressEvent {
            name,
            address1: addr.address1.clone().unwrap_or_default(),
            address2: addr.address2.clone(),
            city: addr.city.clone().unwrap_or_default(),
            province: addr.province_code.clone().unwrap_or_default(),
            zip: addr.zip.clone().unwrap_or_default(),
            country: addr.country.clone().unwrap_or_default(),
        }
    })
}

/// Errors from poller operations.
#[derive(Debug, thiserror::Error)]
pub enum PollerError {
    /// Klaviyo API call failed.
    #[error("Klaviyo error: {0}")]
    Klaviyo(#[from] naked_pineapple_services::klaviyo::KlaviyoError),

    /// Database operation failed.
    #[error("database error: {0}")]
    Database(#[from] crate::db::RepositoryError),
}

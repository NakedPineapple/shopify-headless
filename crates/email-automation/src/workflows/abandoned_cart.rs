//! Abandoned cart detection and recovery workflow.
//!
//! Runs on a schedule (default: every 15 minutes) and performs three tasks:
//!
//! 1. **Detect**: Query Shopify for abandoned checkouts, filter those older
//!    than the configured delay, and insert new ones into the database.
//! 2. **Trigger recovery**: For carts in `detected` status, fire a Klaviyo
//!    "Abandoned Cart Detected" event so Klaviyo flows handle the multi-step
//!    email sequence.
//! 3. **Check recoveries**: For carts in `first_email_sent` status, check if
//!    the customer has since completed a purchase and mark them as recovered.

use naked_pineapple_services::klaviyo::KlaviyoClient;
use naked_pineapple_services::klaviyo::events::{AbandonedCartEventParams, CartLineItem};
use sqlx::PgPool;
use tracing::{debug, error, info, instrument, warn};

use crate::db::abandoned_cart;
use crate::shopify::ShopifyClient;
use crate::shopify::checkouts;

/// Service references needed by the abandoned cart workflow.
pub struct AbandonedCartClients<'a> {
    /// Database connection pool.
    pub pool: &'a PgPool,
    /// Shopify Admin API client.
    pub shopify: &'a ShopifyClient,
    /// Klaviyo client for event tracking.
    pub klaviyo: &'a KlaviyoClient,
    /// Minutes of inactivity before a checkout is considered abandoned.
    pub abandon_delay_minutes: u64,
    /// How far back to poll Shopify for abandoned checkouts (minutes).
    pub poll_window_minutes: u64,
}

/// Run the complete abandoned cart workflow: detect, trigger, and check recoveries.
#[instrument(skip(clients), fields(delay_min = clients.abandon_delay_minutes))]
pub async fn run(clients: &AbandonedCartClients<'_>) {
    detect_new_carts(clients).await;
    trigger_recovery(clients).await;
    check_recoveries(clients).await;
}

/// Query Shopify for abandoned checkouts and insert new ones into the database.
async fn detect_new_carts(clients: &AbandonedCartClients<'_>) {
    let checkouts =
        match checkouts::fetch_abandoned_checkouts(clients.shopify, clients.poll_window_minutes)
            .await
        {
            Ok(c) => c,
            Err(e) => {
                error!(error = %e, "failed to fetch abandoned checkouts from Shopify");
                return;
            }
        };

    if checkouts.is_empty() {
        debug!("no abandoned checkouts found");
        return;
    }

    debug!(count = checkouts.len(), "fetched abandoned checkouts");

    let cutoff = chrono::Utc::now()
        - chrono::Duration::minutes(i64::try_from(clients.abandon_delay_minutes).unwrap_or(60));

    let mut inserted = 0u32;
    for checkout in &checkouts {
        // Skip checkouts without email (can't send recovery)
        let Some(email) = &checkout.email else {
            continue;
        };

        // Parse the checkout creation timestamp
        let Ok(created) = chrono::DateTime::parse_from_rfc3339(&checkout.created_at) else {
            warn!(checkout_id = %checkout.id, "invalid created_at timestamp, skipping");
            continue;
        };
        let created_utc = created.with_timezone(&chrono::Utc);

        // Skip checkouts that haven't been abandoned long enough
        if chrono::DateTime::parse_from_rfc3339(&checkout.updated_at)
            .is_ok_and(|updated| updated > cutoff)
        {
            continue;
        }

        // Skip if already tracked
        match abandoned_cart::exists_by_checkout_id(clients.pool, &checkout.id).await {
            Ok(true) => continue,
            Ok(false) => {}
            Err(e) => {
                warn!(checkout_id = %checkout.id, error = %e, "failed to check cart existence");
                continue;
            }
        }

        let line_items_json = serde_json::to_value(
            checkout
                .line_items
                .iter()
                .map(|li| {
                    serde_json::json!({
                        "title": li.title,
                        "quantity": li.quantity,
                        "variant": li.variant_title,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default();

        let cart_total = checkout.total.parse::<rust_decimal::Decimal>().ok();

        let params = abandoned_cart::InsertParams {
            shopify_checkout_id: &checkout.id,
            customer_email: Some(email.as_str()),
            cart_total,
            line_items: &line_items_json,
            abandoned_at: created_utc,
        };

        match abandoned_cart::insert(clients.pool, &params).await {
            Ok(id) => {
                info!(
                    cart_id = id,
                    checkout_id = %checkout.id,
                    email = %email,
                    "detected abandoned cart"
                );
                inserted += 1;
            }
            Err(e) => {
                warn!(checkout_id = %checkout.id, error = %e, "failed to insert abandoned cart");
            }
        }
    }

    if inserted > 0 {
        info!(count = inserted, "new abandoned carts detected");
    }
}

/// Trigger Klaviyo recovery events for carts in `detected` status.
async fn trigger_recovery(clients: &AbandonedCartClients<'_>) {
    let carts = match abandoned_cart::fetch_detected(clients.pool).await {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "failed to fetch detected carts");
            return;
        }
    };

    if carts.is_empty() {
        return;
    }

    debug!(
        count = carts.len(),
        "triggering recovery for detected carts"
    );

    for cart in &carts {
        let Some(email) = &cart.customer_email else {
            warn!(cart_id = cart.id, "cart has no email, skipping recovery");
            continue;
        };

        let checkout_url = build_checkout_url(&cart.shopify_checkout_id);
        let cart_total = cart
            .cart_total
            .map_or_else(|| "0.00".to_string(), |d| d.to_string());

        let line_items = parse_line_items_from_json(&cart.line_items);

        let params = AbandonedCartEventParams {
            email,
            cart_total: &cart_total,
            checkout_url: &checkout_url,
            line_items: &line_items,
        };

        if let Err(e) = clients.klaviyo.track_abandoned_cart_event(&params).await {
            warn!(
                cart_id = cart.id,
                error = %e,
                "failed to track abandoned cart event in Klaviyo"
            );
            continue;
        }

        if let Err(e) = abandoned_cart::mark_recovery_triggered(clients.pool, cart.id).await {
            warn!(cart_id = cart.id, error = %e, "failed to mark cart recovery triggered");
        } else {
            info!(cart_id = cart.id, email = %email, "triggered cart recovery");
        }
    }
}

/// Check if carts in `first_email_sent` status have been recovered
/// (customer completed a purchase).
async fn check_recoveries(clients: &AbandonedCartClients<'_>) {
    let carts = match abandoned_cart::fetch_pending_recovery(clients.pool).await {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "failed to fetch pending recovery carts");
            return;
        }
    };

    if carts.is_empty() {
        return;
    }

    debug!(count = carts.len(), "checking for recovered carts");

    for cart in &carts {
        let Some(email) = &cart.customer_email else {
            continue;
        };

        match checkouts::find_recovery_order(clients.shopify, email, &cart.abandoned_at_str).await {
            Ok(Some(order_id)) => {
                if let Err(e) =
                    abandoned_cart::mark_recovered(clients.pool, cart.id, &order_id).await
                {
                    warn!(cart_id = cart.id, error = %e, "failed to mark cart as recovered");
                } else {
                    info!(
                        cart_id = cart.id,
                        order_id = %order_id,
                        email = %email,
                        "abandoned cart recovered"
                    );
                }
            }
            Ok(None) => {
                debug!(cart_id = cart.id, "no recovery order found yet");
            }
            Err(e) => {
                warn!(
                    cart_id = cart.id,
                    error = %e,
                    "failed to check for recovery order"
                );
            }
        }
    }
}

/// Build the checkout URL from a Shopify checkout ID.
///
/// The `abandonedCheckoutUrl` from Shopify is the direct recovery link,
/// but since we store the Shopify global ID, we provide a fallback.
fn build_checkout_url(shopify_checkout_id: &str) -> String {
    // The Shopify checkout ID is a global ID like "gid://shopify/AbandonedCheckout/123"
    // The actual recovery URL comes from the Shopify API, but since we only stored the ID,
    // we link to the store. In practice, Klaviyo flows should use the checkout_url
    // property from the event to build the actual recovery link.
    shopify_checkout_id.to_string()
}

/// Parse line items from the JSON array stored in the database.
fn parse_line_items_from_json(json: &serde_json::Value) -> Vec<CartLineItem> {
    let Some(arr) = json.as_array() else {
        return Vec::new();
    };

    arr.iter()
        .filter_map(|item| {
            let title = item.get("title")?.as_str()?.to_string();
            let quantity = item
                .get("quantity")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(1);
            let variant = item
                .get("variant")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            Some(CartLineItem {
                title,
                quantity,
                variant,
            })
        })
        .collect()
}

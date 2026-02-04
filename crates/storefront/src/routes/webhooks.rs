//! Shopify webhook handlers.
//!
//! Receives order events from Shopify and forwards them to Mixpanel
//! for server-side purchase tracking.
//!
//! # Authentication
//!
//! Webhooks are verified using HMAC-SHA256 signatures. Shopify signs each
//! request body with the webhook secret and sends the signature in the
//! `X-Shopify-Hmac-Sha256` header.

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use hmac::{Hmac, Mac};
use secrecy::ExposeSecret;
use serde::Deserialize;
use sha2::Sha256;
use tracing::{debug, info, warn};

use crate::services::MixpanelClient;
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

/// Verify the Shopify HMAC-SHA256 signature.
fn verify_shopify_hmac(secret: &str, body: &[u8], signature: &str) -> bool {
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);

    let Ok(expected) = base64::engine::general_purpose::STANDARD.decode(signature) else {
        return false;
    };
    mac.verify_slice(&expected).is_ok()
}

/// Shopify order webhook payload (subset of fields we need).
#[derive(Debug, Deserialize)]
struct ShopifyOrder {
    /// Order ID.
    id: u64,
    /// Order name/number (e.g., "#1001").
    name: String,
    /// Total price as string (e.g., "49.99").
    total_price: String,
    /// Shipping cost as string.
    #[serde(default)]
    total_shipping_price_set: Option<ShopifyMoneySet>,
    /// Tax amount as string.
    #[serde(default)]
    total_tax: Option<String>,
    /// Line items.
    #[serde(default)]
    line_items: Vec<ShopifyLineItem>,
    /// Customer info (may be null for guest checkout).
    customer: Option<ShopifyCustomer>,
}

#[derive(Debug, Deserialize)]
struct ShopifyMoneySet {
    shop_money: Option<ShopifyMoney>,
}

#[derive(Debug, Deserialize)]
struct ShopifyMoney {
    amount: String,
}

#[derive(Debug, Deserialize)]
struct ShopifyLineItem {
    product_id: Option<u64>,
    title: String,
    quantity: u32,
    price: String,
}

#[derive(Debug, Deserialize)]
struct ShopifyCustomer {
    id: u64,
    email: Option<String>,
}

/// Handle Shopify `orders/create` webhook.
///
/// Verifies the HMAC signature, parses the order payload, and sends
/// an `Order Completed` event to Mixpanel with revenue tracking.
///
/// # Route
///
/// `POST /api/webhooks/shopify/orders-create`
pub async fn orders_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let config = state.config();

    // Check if webhook secret is configured
    let Some(secret) = &config.shopify_webhook_secret else {
        debug!("Shopify webhook secret not configured, ignoring webhook");
        return StatusCode::OK;
    };

    // Verify HMAC signature
    let signature = headers
        .get("X-Shopify-Hmac-Sha256")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !verify_shopify_hmac(secret.expose_secret(), &body, signature) {
        warn!("Shopify webhook HMAC verification failed");
        return StatusCode::UNAUTHORIZED;
    }

    debug!("Shopify webhook HMAC verified");

    // Parse order payload
    let order: ShopifyOrder = match serde_json::from_slice(&body) {
        Ok(order) => order,
        Err(e) => {
            warn!(error = %e, "Failed to parse Shopify order webhook payload");
            // Return 200 so Shopify doesn't retry
            return StatusCode::OK;
        }
    };

    // Need a customer to identify the Mixpanel user
    let Some(customer) = &order.customer else {
        debug!(order_name = %order.name, "Order has no customer, skipping Mixpanel tracking");
        return StatusCode::OK;
    };

    // Check if Mixpanel is configured
    let Some(token) = &config.analytics.mixpanel_project_token else {
        debug!("Mixpanel not configured, skipping purchase tracking");
        return StatusCode::OK;
    };

    let distinct_id = format!("gid://shopify/Customer/{}", customer.id);
    let revenue: f64 = order.total_price.parse().unwrap_or(0.0);
    let shipping: f64 = order
        .total_shipping_price_set
        .as_ref()
        .and_then(|s| s.shop_money.as_ref())
        .map_or(0.0, |m| m.amount.parse().unwrap_or(0.0));
    let tax: f64 = order
        .total_tax
        .as_ref()
        .map_or(0.0, |t| t.parse().unwrap_or(0.0));
    let product_count = order.line_items.len();

    let client = MixpanelClient::new(token.clone());

    // Track Order Completed event
    client
        .track(
            &distinct_id,
            "Order Completed",
            serde_json::json!({
                "Order ID": order.name,
                "Revenue": revenue,
                "Shipping": shipping,
                "Tax": tax,
                "Products": product_count,
            }),
        )
        .await;

    // Track revenue for lifetime value
    client.track_charge(&distinct_id, revenue).await;

    info!(
        order_name = %order.name,
        revenue,
        products = product_count,
        "Tracked purchase event in Mixpanel"
    );

    StatusCode::OK
}

use base64::Engine;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_shopify_hmac_valid() {
        let secret = "test-secret";
        let body = b"test body";

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .expect("HMAC key creation should not fail in test");
        mac.update(body);
        let signature =
            base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

        assert!(verify_shopify_hmac(secret, body, &signature));
    }

    #[test]
    fn test_verify_shopify_hmac_invalid() {
        let secret = "test-secret";
        let body = b"test body";
        let bad_signature = base64::engine::general_purpose::STANDARD.encode(b"bad signature");

        assert!(!verify_shopify_hmac(secret, body, &bad_signature));
    }

    #[test]
    fn test_verify_shopify_hmac_invalid_base64() {
        assert!(!verify_shopify_hmac(
            "secret",
            b"body",
            "not-valid-base64!!!"
        ));
    }
}

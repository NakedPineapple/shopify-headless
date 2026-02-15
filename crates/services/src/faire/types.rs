//! Faire Brand API v2 request/response types.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Credentials
// ---------------------------------------------------------------------------

/// Faire API credentials (API key auth, no OAuth).
#[derive(Clone)]
pub struct FaireCredentials {
    /// Faire brand identifier.
    pub brand_id: String,
    /// Faire brand API token.
    pub api_token: secrecy::SecretString,
}

impl std::fmt::Debug for FaireCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FaireCredentials")
            .field("brand_id", &self.brand_id)
            .field("api_token", &"[REDACTED]")
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Brand Info
// ---------------------------------------------------------------------------

/// Faire brand account information.
#[derive(Debug, Clone, Deserialize)]
pub struct BrandInfo {
    pub token: Option<String>,
    pub name: Option<String>,
    pub brand_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Product Types
// ---------------------------------------------------------------------------

/// A product from Faire.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FaireProduct {
    pub token: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub brand_token: Option<String>,
    pub wholesale_price_cents: Option<i64>,
    pub retail_price_cents: Option<i64>,
    pub active: Option<bool>,
    pub unit_multiplier: Option<i32>,
    pub minimum_order_quantity: Option<i32>,
    pub image_url: Option<String>,
}

/// Paginated response for products.
#[derive(Debug, Deserialize)]
pub struct ProductsPage {
    pub products: Option<Vec<FaireProduct>>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
    pub has_more: Option<bool>,
}

// ---------------------------------------------------------------------------
// Order Types
// ---------------------------------------------------------------------------

/// A wholesale order from Faire.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FaireOrder {
    pub token: Option<String>,
    pub state: Option<String>,
    pub ship_after: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub address: Option<FaireAddress>,
    pub retailer: Option<RetailerInfo>,
    pub items: Option<Vec<OrderItem>>,
    pub payout_costs: Option<PayoutCosts>,
    pub payment_initiated_at: Option<String>,
    pub source: Option<String>,
}

/// Retailer info on a Faire order.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetailerInfo {
    pub token: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

/// Address on a Faire order.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FaireAddress {
    pub name: Option<String>,
    pub address1: Option<String>,
    pub address2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
}

/// Line item in a Faire order.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderItem {
    pub token: Option<String>,
    pub product_token: Option<String>,
    pub product_option_token: Option<String>,
    pub product_name: Option<String>,
    pub quantity: Option<i32>,
    pub price_cents: Option<i64>,
    pub sku: Option<String>,
}

/// Payout cost breakdown on a Faire order.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PayoutCosts {
    pub total_payout_cents: Option<i64>,
    pub commission_cents: Option<i64>,
    pub shipping_cents: Option<i64>,
    pub total_order_cents: Option<i64>,
}

/// Paginated response for orders.
#[derive(Debug, Deserialize)]
pub struct OrdersPage {
    pub orders: Option<Vec<FaireOrder>>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
    pub has_more: Option<bool>,
}

// ---------------------------------------------------------------------------
// Return Types
// ---------------------------------------------------------------------------

/// A return from Faire.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FaireReturn {
    pub token: Option<String>,
    pub order_token: Option<String>,
    pub state: Option<String>,
    pub reason: Option<String>,
    pub retailer_note: Option<String>,
    pub refund_total_cents: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub decision_deadline: Option<String>,
}

/// Paginated response for returns.
#[derive(Debug, Deserialize)]
pub struct ReturnsPage {
    pub returns: Option<Vec<FaireReturn>>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
    pub has_more: Option<bool>,
}

// ---------------------------------------------------------------------------
// Payout Types
// ---------------------------------------------------------------------------

/// A payout from Faire.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FairePayout {
    pub token: Option<String>,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub total_order_cents: Option<i64>,
    pub total_refund_cents: Option<i64>,
    pub total_commission_cents: Option<i64>,
    pub total_shipping_cents: Option<i64>,
    pub net_payout_cents: Option<i64>,
    pub currency: Option<String>,
    pub state: Option<String>,
    pub paid_at: Option<String>,
    pub line_items: Option<Vec<PayoutLineItem>>,
}

/// A line item within a payout.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PayoutLineItem {
    pub order_token: Option<String>,
    pub order_amount_cents: Option<i64>,
    pub refund_amount_cents: Option<i64>,
    pub commission_cents: Option<i64>,
    pub shipping_fee_cents: Option<i64>,
    pub net_amount_cents: Option<i64>,
}

/// Paginated response for payouts.
#[derive(Debug, Deserialize)]
pub struct PayoutsPage {
    pub payouts: Option<Vec<FairePayout>>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
    pub has_more: Option<bool>,
}

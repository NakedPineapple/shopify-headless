//! Meta Commerce API request/response types.

use serde::{Deserialize, Serialize};

/// Meta Commerce credentials for Graph API access.
#[derive(Clone)]
pub struct MetaCommerceCredentials {
    /// Facebook App ID.
    pub app_id: String,
    /// Facebook App Secret.
    pub app_secret: secrecy::SecretString,
    /// Page Access Token (long-lived, 60-day expiry).
    pub page_access_token: secrecy::SecretString,
    /// Facebook Page ID.
    pub page_id: String,
    /// Commerce Account ID.
    pub commerce_account_id: String,
    /// Product Catalog ID.
    pub catalog_id: String,
}

impl std::fmt::Debug for MetaCommerceCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetaCommerceCredentials")
            .field("app_id", &self.app_id)
            .field("app_secret", &"[REDACTED]")
            .field("page_access_token", &"[REDACTED]")
            .field("page_id", &self.page_id)
            .field("commerce_account_id", &self.commerce_account_id)
            .field("catalog_id", &self.catalog_id)
            .finish()
    }
}

/// Cached page access token with expiry.
#[derive(Debug, Clone)]
pub struct PageAccessToken {
    pub access_token: String,
    pub expires_at: i64,
}

impl PageAccessToken {
    /// Check if the token is expired (with 300-second buffer for 60-day tokens).
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now >= self.expires_at - 300
    }
}

/// Token exchange response from the Graph API.
#[derive(Debug, Deserialize)]
pub struct TokenExchangeResponse {
    pub access_token: String,
    pub token_type: Option<String>,
    /// Seconds until expiry (typically 5,184,000 = 60 days).
    pub expires_in: Option<i64>,
}

// ---------------------------------------------------------------------------
// Graph API Error Types
// ---------------------------------------------------------------------------

/// Graph API error response wrapper.
#[derive(Debug, Deserialize)]
pub struct GraphApiErrorResponse {
    pub error: GraphApiError,
}

/// A single Graph API error.
#[derive(Debug, Deserialize)]
pub struct GraphApiError {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub code: Option<i32>,
    pub error_subcode: Option<i32>,
    pub fbtrace_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Commerce Account / Shop Types
// ---------------------------------------------------------------------------

/// Commerce account information from the Graph API.
#[derive(Debug, Clone, Deserialize)]
pub struct CommerceAccountInfo {
    pub id: String,
    pub name: Option<String>,
}

/// Shop information from the Graph API.
#[derive(Debug, Clone, Deserialize)]
pub struct ShopInfo {
    pub id: String,
    pub name: Option<String>,
    pub shop_status: Option<String>,
}

// ---------------------------------------------------------------------------
// Catalog / Product Types
// ---------------------------------------------------------------------------

/// A product from the Meta catalog.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FacebookProduct {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub price: Option<String>,
    pub currency: Option<String>,
    pub image_url: Option<String>,
    pub url: Option<String>,
    pub retailer_id: Option<String>,
    pub availability: Option<String>,
    pub brand: Option<String>,
    pub category: Option<String>,
}

/// Paginated response for catalog products.
#[derive(Debug, Deserialize)]
pub struct ProductsPage {
    pub data: Vec<FacebookProduct>,
    pub paging: Option<GraphPaging>,
}

// ---------------------------------------------------------------------------
// Order Types
// ---------------------------------------------------------------------------

/// An order from Meta Commerce.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FacebookOrder {
    pub id: String,
    pub order_status: Option<OrderStatus>,
    pub created: Option<String>,
    pub last_updated: Option<String>,
    pub channel: Option<String>,
    pub selected_shipping_option: Option<ShippingOption>,
    pub shipping_address: Option<FacebookShippingAddress>,
    pub estimated_payment_details: Option<EstimatedPaymentDetails>,
    pub buyer_details: Option<BuyerDetails>,
    pub items: Option<OrderItemsData>,
}

/// Order status wrapper.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderStatus {
    pub state: Option<String>,
}

/// Shipping option selected for an order.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShippingOption {
    pub name: Option<String>,
    pub price: Option<FacebookMoney>,
    pub calculated_tax: Option<FacebookMoney>,
}

/// Shipping address for an order.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FacebookShippingAddress {
    pub name: Option<String>,
    pub street1: Option<String>,
    pub street2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
}

/// Payment details for an order.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EstimatedPaymentDetails {
    pub subtotal: Option<FacebookMoney>,
    pub tax: Option<FacebookMoney>,
    pub total_amount: Option<FacebookMoney>,
    pub shipping: Option<FacebookMoney>,
}

/// Monetary amount with currency.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FacebookMoney {
    pub amount: Option<String>,
    pub currency: Option<String>,
}

/// Buyer details.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BuyerDetails {
    pub name: Option<String>,
    pub email: Option<String>,
}

/// Wrapper for order items data.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderItemsData {
    pub data: Vec<FacebookOrderItem>,
}

/// A single order line item.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FacebookOrderItem {
    pub id: Option<String>,
    pub product_id: Option<String>,
    pub retailer_id: Option<String>,
    pub quantity: Option<i32>,
    pub price_per_unit: Option<FacebookMoney>,
    pub tax_details: Option<TaxDetails>,
}

/// Tax details for an order item.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaxDetails {
    pub estimated_tax: Option<FacebookMoney>,
}

/// Paginated response for orders.
#[derive(Debug, Deserialize)]
pub struct OrdersPage {
    pub data: Vec<FacebookOrder>,
    pub paging: Option<GraphPaging>,
}

// ---------------------------------------------------------------------------
// Graph API Pagination
// ---------------------------------------------------------------------------

/// Graph API cursor-based pagination.
#[derive(Debug, Clone, Deserialize)]
pub struct GraphPaging {
    pub cursors: Option<GraphCursors>,
    pub next: Option<String>,
    pub previous: Option<String>,
}

/// Cursor values for Graph API pagination.
#[derive(Debug, Clone, Deserialize)]
pub struct GraphCursors {
    pub before: Option<String>,
    pub after: Option<String>,
}

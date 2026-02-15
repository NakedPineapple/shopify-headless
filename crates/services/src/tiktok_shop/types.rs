//! TikTok Shop Open API request/response types.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// API Envelope
// ---------------------------------------------------------------------------

/// TikTok API response envelope: `{ code, message, data }`.
#[derive(Debug, Deserialize)]
pub struct TikTokApiResponse<T> {
    pub code: i32,
    pub message: String,
    pub data: Option<T>,
}

// ---------------------------------------------------------------------------
// Credentials / Auth
// ---------------------------------------------------------------------------

/// TikTok Shop credentials for Open API v2 access.
#[derive(Clone)]
pub struct TikTokShopCredentials {
    /// Application key.
    pub app_key: String,
    /// Application secret (used for HMAC-SHA256 request signing).
    pub app_secret: secrecy::SecretString,
    /// OAuth 2.0 access token (24-hour expiry).
    pub access_token: secrecy::SecretString,
    /// OAuth 2.0 refresh token (no expiry).
    pub refresh_token: secrecy::SecretString,
    /// Authorized shop ID.
    pub shop_id: String,
    /// Shop cipher for API requests.
    pub shop_cipher: String,
}

impl std::fmt::Debug for TikTokShopCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TikTokShopCredentials")
            .field("app_key", &self.app_key)
            .field("app_secret", &"[REDACTED]")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("shop_id", &self.shop_id)
            .field("shop_cipher", &self.shop_cipher)
            .finish()
    }
}

/// Cached access token with expiry.
#[derive(Debug, Clone)]
pub struct AccessToken {
    pub access_token: String,
    pub expires_at: i64,
}

impl AccessToken {
    /// Check if the token is expired (with 300-second buffer for 24hr tokens).
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now >= self.expires_at - 300
    }
}

/// Token refresh response from TikTok.
#[derive(Debug, Deserialize)]
pub struct TokenRefreshResponse {
    pub access_token: String,
    pub access_token_expire_in: i64,
    pub refresh_token: String,
    pub refresh_token_expire_in: i64,
}

// ---------------------------------------------------------------------------
// Shop Info
// ---------------------------------------------------------------------------

/// Authorized shop information.
#[derive(Debug, Clone, Deserialize)]
pub struct ShopInfo {
    pub id: Option<String>,
    pub name: Option<String>,
    pub region: Option<String>,
    pub seller_type: Option<String>,
}

/// Response data for authorized shop listing.
#[derive(Debug, Clone, Deserialize)]
pub struct ShopInfoData {
    pub shops: Option<Vec<ShopInfo>>,
}

// ---------------------------------------------------------------------------
// Catalog / Product
// ---------------------------------------------------------------------------

/// A product from the TikTok Shop catalog.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TikTokProduct {
    pub id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub main_images: Option<Vec<ProductImage>>,
    pub status: Option<String>,
    pub skus: Option<Vec<TikTokSku>>,
    pub category_id: Option<String>,
}

/// Product image with multiple URLs.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProductImage {
    pub urls: Option<Vec<String>>,
}

/// A SKU (stock keeping unit) within a product.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TikTokSku {
    pub id: Option<String>,
    pub seller_sku: Option<String>,
    pub price: Option<TikTokPrice>,
    pub inventory: Option<Vec<SkuInventory>>,
}

/// Price information for a SKU.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TikTokPrice {
    pub sale_price: Option<String>,
    pub original_price: Option<String>,
    pub currency: Option<String>,
}

/// Inventory for a SKU at a specific warehouse.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SkuInventory {
    pub warehouse_id: Option<String>,
    pub quantity: Option<i32>,
}

/// Paginated product search response.
#[derive(Debug, Deserialize)]
pub struct ProductSearchData {
    pub products: Option<Vec<TikTokProduct>>,
    pub total_count: Option<i32>,
    pub next_page_token: Option<String>,
}

// ---------------------------------------------------------------------------
// Orders
// ---------------------------------------------------------------------------

/// A TikTok Shop order.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TikTokOrder {
    pub id: Option<String>,
    pub status: Option<String>,
    pub create_time: Option<i64>,
    pub update_time: Option<i64>,
    // Buyer
    pub buyer_uid: Option<String>,
    pub buyer_message: Option<String>,
    pub recipient_address: Option<RecipientAddress>,
    // Payment
    pub payment: Option<PaymentInfo>,
    // Content attribution
    pub source_type: Option<String>,
    // Creator / affiliate
    pub creator: Option<CreatorInfo>,
    pub is_affiliate_order: Option<bool>,
    pub commission: Option<CommissionInfo>,
    // Fulfillment
    pub fulfillment_type: Option<String>,
    pub fbt_warehouse_id: Option<String>,
    pub shipping: Option<ShippingInfo>,
    // Items
    pub line_items: Option<Vec<OrderLineItem>>,
}

/// Recipient address for an order.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecipientAddress {
    pub full_address: Option<String>,
    pub name: Option<String>,
    pub phone_number: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub zipcode: Option<String>,
    pub country: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
}

/// Payment details for an order.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaymentInfo {
    pub total_amount: Option<String>,
    pub currency: Option<String>,
    pub shipping_fee: Option<String>,
    pub platform_discount: Option<String>,
    pub seller_discount: Option<String>,
}

/// TikTok creator (influencer) info for an order.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreatorInfo {
    pub username: Option<String>,
    pub id: Option<String>,
}

/// Affiliate commission information.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommissionInfo {
    pub rate: Option<String>,
    pub amount: Option<String>,
    pub status: Option<String>,
}

/// Shipping / tracking information.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShippingInfo {
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub tracking_number: Option<String>,
    pub status: Option<String>,
}

/// A single line item within an order.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderLineItem {
    pub id: Option<String>,
    pub product_id: Option<String>,
    pub sku_id: Option<String>,
    pub product_name: Option<String>,
    pub quantity: Option<i32>,
    pub sale_price: Option<String>,
    pub original_price: Option<String>,
    pub currency: Option<String>,
    pub seller_discount: Option<String>,
    pub platform_discount: Option<String>,
}

/// Paginated order list response.
#[derive(Debug, Deserialize)]
pub struct OrderListData {
    pub orders: Option<Vec<TikTokOrder>>,
    pub total_count: Option<i32>,
    pub next_page_token: Option<String>,
}

// ---------------------------------------------------------------------------
// Settlements / Finance
// ---------------------------------------------------------------------------

/// A settlement from TikTok Shop.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TikTokSettlement {
    pub id: Option<String>,
    pub period_start: Option<i64>,
    pub period_end: Option<i64>,
    pub status: Option<String>,
    pub total_revenue: Option<String>,
    pub total_refunds: Option<String>,
    pub total_platform_fees: Option<String>,
    pub total_affiliate_commission: Option<String>,
    pub net_payout: Option<String>,
    pub currency: Option<String>,
    pub payout_date: Option<i64>,
    pub line_items: Option<Vec<SettlementLineItem>>,
}

/// A single line item within a settlement.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettlementLineItem {
    pub order_id: Option<String>,
    pub order_amount: Option<String>,
    pub refund_amount: Option<String>,
    pub referral_fee: Option<String>,
    pub affiliate_commission: Option<String>,
    pub shipping_fee_subsidy: Option<String>,
    pub net_amount: Option<String>,
}

/// Paginated settlement list response.
#[derive(Debug, Deserialize)]
pub struct SettlementListData {
    pub settlements: Option<Vec<TikTokSettlement>>,
    pub total_count: Option<i32>,
    pub next_page_token: Option<String>,
}

// ---------------------------------------------------------------------------
// Returns / Refunds
// ---------------------------------------------------------------------------

/// A return or refund request from TikTok Shop.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TikTokReturn {
    pub id: Option<String>,
    pub order_id: Option<String>,
    pub status: Option<String>,
    pub return_type: Option<String>,
    pub reason: Option<String>,
    pub buyer_note: Option<String>,
    pub refund_amount: Option<String>,
    pub currency: Option<String>,
    pub decision_deadline: Option<i64>,
    pub tracking_number: Option<String>,
}

/// Paginated return list response.
#[derive(Debug, Deserialize)]
pub struct ReturnListData {
    pub returns: Option<Vec<TikTokReturn>>,
    pub total_count: Option<i32>,
    pub next_page_token: Option<String>,
}

// ---------------------------------------------------------------------------
// Shop Performance
// ---------------------------------------------------------------------------

/// Shop performance / health metrics.
#[derive(Debug, Clone, Deserialize)]
pub struct ShopPerformance {
    pub on_time_delivery_rate: Option<f64>,
    pub late_dispatch_rate: Option<f64>,
    pub seller_fault_cancel_rate: Option<f64>,
    pub customer_satisfaction_rate: Option<f64>,
    pub overall_health: Option<String>,
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

/// Generic pagination metadata.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PageInfo {
    pub total_count: Option<i32>,
    pub next_page_token: Option<String>,
}

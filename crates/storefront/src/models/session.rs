//! Session key constants.

/// Session keys for authentication data.
pub mod keys {
    /// Key for storing the Shopify cart ID.
    pub const CART_ID: &str = "cart_id";

    /// Key for Shopify OAuth state (CSRF protection).
    pub const SHOPIFY_OAUTH_STATE: &str = "shopify_oauth_state";

    /// Key for Shopify OAuth nonce (`OpenID` Connect replay protection).
    pub const SHOPIFY_OAUTH_NONCE: &str = "shopify_oauth_nonce";

    /// Key for Shopify customer access token (Customer Account API OAuth).
    pub const SHOPIFY_CUSTOMER_TOKEN: &str = "shopify_customer_token";
}

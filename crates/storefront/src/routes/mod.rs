//! HTTP route handlers for storefront.
//!
//! # Route Structure
//!
//! ```text
//! GET  /                       - Home page
//! GET  /health                 - Health check
//!
//! # Products
//! GET  /products               - Product listing
//! GET  /products/:handle       - Product detail
//! GET  /products/:handle/quick-view - Quick view fragment (HTMX)
//! GET  /collections            - Collection listing
//! GET  /collections/:handle    - Collection detail
//!
//! # Cart (HTMX fragments)
//! GET  /cart                   - Cart page
//! POST /cart/add               - Add to cart (returns empty, triggers cartUpdated)
//! POST /cart/update            - Update quantity (returns cart_items fragment)
//! POST /cart/remove            - Remove item (returns cart_items fragment)
//! GET  /cart/count             - Cart count badge (fragment)
//! GET  /cart/summary           - Order summary with promotions (fragment)
//! POST /cart/claim-gwp         - Claim gift-with-purchase item
//!
//! # Checkout
//! GET  /checkout               - Redirect to Shopify checkout
//!
//! # Newsletter
//! POST /newsletter/subscribe   - Subscribe to newsletter (HTMX fragment)
//!
//! # Contact
//! GET  /contact                  - Contact page
//! POST /contact/product-question - Submit product question (JSON API)
//!
//! # Shopify Customer OAuth
//! GET  /auth/shopify/login     - Redirect to Shopify OAuth
//! GET  /auth/shopify/callback  - Handle OAuth callback
//! POST /auth/shopify/logout    - Logout from Shopify
//!
//! # Account (requires Shopify OAuth)
//! GET  /account                          - Account overview
//! GET  /account/profile                  - Profile editing
//! POST /account/profile                  - Update profile
//! GET  /account/orders                   - Order history
//! GET  /account/orders/:id               - Order detail
//! GET  /account/orders/:id/return        - Return request form
//! POST /account/orders/:id/return        - Submit return request
//! GET  /account/addresses                - Address list
//! GET  /account/subscriptions            - Subscription list
//! GET  /account/subscriptions/:id        - Subscription detail
//! POST /account/subscriptions/:id/skip/:idx   - Skip billing cycle
//! POST /account/subscriptions/:id/unskip/:idx - Unskip billing cycle
//! ```

pub mod account;
pub mod auth;
pub mod blog;
pub mod cart;
pub mod collections;
pub mod contact;
pub mod home;
pub mod images;
pub mod manifest;
pub mod newsletter;
pub mod pages;
pub mod products;
pub mod search;
pub mod shopify_auth;
pub mod webhooks;
pub mod well_known;

use axum::{
    Router,
    routing::{get, post},
};

use crate::middleware::{api_rate_limiter, auth_rate_limiter, gwp_rate_limiter};
use crate::state::AppState;

/// Create the auth routes router.
///
/// Login and logout are rate limited to ~10 requests per minute per IP.
/// The callback is excluded from rate limiting because it is a one-time
/// redirect from Shopify with a short-lived authorization code and cannot
/// be meaningfully abused. Sharing the rate limiter caused the normal
/// OAuth round-trip (login + callback) to consume burst tokens too quickly.
pub fn auth_routes() -> Router<AppState> {
    let rate_limited = Router::new()
        .route("/shopify/login", get(shopify_auth::login))
        .route("/shopify/logout", post(shopify_auth::logout))
        .layer(auth_rate_limiter());

    Router::new()
        .route("/shopify/callback", get(shopify_auth::callback))
        .merge(rate_limited)
}

/// Create the product routes router.
pub fn product_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(products::index))
        .route("/{handle}", get(products::show))
        .route("/{handle}/quick-view", get(products::quick_view))
}

/// Create the collection routes router.
pub fn collection_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(collections::index))
        .route("/{handle}", get(collections::show))
}

/// Create the cart routes router.
///
/// Rate limited to ~100 requests per minute per IP to prevent cart abuse.
/// The GWP claim endpoint has stricter rate limiting (~5/min) to prevent abuse.
pub fn cart_routes() -> Router<AppState> {
    // GWP claim route with stricter rate limiting
    let gwp_route = Router::new()
        .route("/claim-gwp", post(cart::claim_gwp))
        .layer(gwp_rate_limiter());

    // Other cart routes with standard API rate limiting
    Router::new()
        .route("/", get(cart::show))
        .route("/add", post(cart::add))
        .route("/update", post(cart::update))
        .route("/remove", post(cart::remove))
        .route("/count", get(cart::count))
        .route("/summary", get(cart::summary))
        .merge(gwp_route)
        .layer(api_rate_limiter())
}

/// Create the account routes router.
pub fn account_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(account::index))
        .route(
            "/profile",
            get(account::profile_page).post(account::update_profile),
        )
        .route("/orders", get(account::orders))
        .route("/orders/{id}", get(account::order_detail))
        .route(
            "/orders/{id}/return",
            get(account::return_form).post(account::request_return),
        )
        .route(
            "/addresses",
            get(account::addresses).post(account::create_address),
        )
        .route("/addresses/new", get(account::new_address))
        .route(
            "/addresses/{id}",
            post(account::update_address).delete(account::delete_address),
        )
        .route("/addresses/{id}/edit", get(account::edit_address))
        .route("/subscriptions", get(account::subscriptions))
        .route("/subscriptions/{id}", get(account::subscription_detail))
        .route(
            "/subscriptions/{id}/pause",
            post(account::pause_subscription),
        )
        .route(
            "/subscriptions/{id}/cancel",
            post(account::cancel_subscription),
        )
        .route(
            "/subscriptions/{id}/activate",
            post(account::activate_subscription),
        )
        .route(
            "/subscriptions/{id}/skip/{cycle_index}",
            post(account::skip_billing_cycle),
        )
        .route(
            "/subscriptions/{id}/unskip/{cycle_index}",
            post(account::unskip_billing_cycle),
        )
}

/// Create all routes for the storefront.
pub fn routes() -> Router<AppState> {
    Router::new()
        // Home page
        .route("/", get(home::home))
        // Web app manifest
        .route("/manifest.webmanifest", get(manifest::webmanifest))
        // Well-known endpoints (GPC, security.txt)
        .nest("/.well-known", well_known::router())
        // Shopify image proxy (for Cloudflare Image Resizing)
        .route("/images/shopify/{*path}", get(images::proxy_shopify_image))
        // cdn-cgi fallback for local dev (Cloudflare intercepts in production)
        .route("/cdn-cgi/image/{*rest}", get(images::cdn_cgi_fallback))
        // Product routes
        .nest("/products", product_routes())
        // Collection routes
        .nest("/collections", collection_routes())
        // Blog routes
        .nest("/blog", blog::router())
        // Static content pages
        .merge(pages::router())
        // Cart routes
        .nest("/cart", cart_routes())
        // Checkout redirect
        .route("/checkout", get(cart::checkout))
        // Search routes
        .nest("/search", search::router())
        // Account routes
        .nest("/account", account_routes())
        // Auth routes
        .nest("/auth", auth_routes())
        // Newsletter routes
        .route("/newsletter/subscribe", post(newsletter::subscribe))
        .route(
            "/newsletter/unsubscribe",
            get(newsletter::unsubscribe_page).post(newsletter::unsubscribe),
        )
        // Contact routes
        .route("/contact/product-question", post(contact::product_question))
        // Shopify webhooks (CSRF-exempt via /api/webhooks prefix)
        .route(
            "/api/webhooks/shopify/orders-create",
            post(webhooks::orders_create),
        )
}

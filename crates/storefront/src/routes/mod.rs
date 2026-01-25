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
//!
//! # Checkout
//! GET  /checkout               - Redirect to Shopify checkout
//!
//! # Auth
//! GET  /auth/login             - Login page
//! POST /auth/login             - Login action
//! GET  /auth/register          - Register page
//! POST /auth/register          - Register action
//! POST /auth/logout            - Logout action
//!
//! # `WebAuthn` API
//! POST /api/auth/webauthn/register/start      - Start passkey registration
//! POST /api/auth/webauthn/register/finish     - Finish passkey registration
//! POST /api/auth/webauthn/authenticate/start  - Start passkey authentication
//! POST /api/auth/webauthn/authenticate/finish - Finish passkey authentication
//!
//! # Account (requires auth)
//! GET  /account                - Account overview
//! GET  /account/passkeys       - Passkey management
//! ```

pub mod account;
pub mod api;
pub mod auth;
pub mod cart;
pub mod collections;
pub mod home;
pub mod products;

use axum::{
    Router,
    routing::{get, post},
};

use crate::state::AppState;

/// Create the auth routes router.
pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/login", get(auth::login_page).post(auth::login))
        .route("/register", get(auth::register_page).post(auth::register))
        .route("/logout", post(auth::logout))
}

/// Create the `WebAuthn` API routes router.
pub fn webauthn_api_routes() -> Router<AppState> {
    Router::new()
        .route("/register/start", post(api::webauthn::start_registration))
        .route("/register/finish", post(api::webauthn::finish_registration))
        .route(
            "/authenticate/start",
            post(api::webauthn::start_authentication),
        )
        .route(
            "/authenticate/finish",
            post(api::webauthn::finish_authentication),
        )
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
pub fn cart_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(cart::show))
        .route("/add", post(cart::add))
        .route("/update", post(cart::update))
        .route("/remove", post(cart::remove))
        .route("/count", get(cart::count))
}

/// Create the account routes router.
pub fn account_routes() -> Router<AppState> {
    Router::new().route("/", get(account::index))
    // TODO: Add passkey management routes
    // .route("/passkeys", get(account::passkeys))
}

/// Create all routes for the storefront.
pub fn routes() -> Router<AppState> {
    Router::new()
        // Home page
        .route("/", get(home::home))
        // Product routes
        .nest("/products", product_routes())
        // Collection routes
        .nest("/collections", collection_routes())
        // Cart routes
        .nest("/cart", cart_routes())
        // Checkout redirect
        .route("/checkout", get(cart::checkout))
        // Account routes (TODO: add auth middleware)
        .nest("/account", account_routes())
        // Auth routes
        .nest("/auth", auth_routes())
        // `WebAuthn` API
        .nest("/api/auth/webauthn", webauthn_api_routes())
}

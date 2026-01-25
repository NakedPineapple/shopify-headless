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
//! GET  /collections            - Collection listing
//! GET  /collections/:handle    - Collection detail
//!
//! # Cart (HTMX fragments)
//! GET  /cart                   - Cart page
//! POST /cart/add               - Add to cart (returns fragment)
//! POST /cart/update            - Update quantity (returns fragment)
//! POST /cart/remove            - Remove item (returns fragment)
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

pub mod api;
pub mod auth;

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

/// Create all routes for the storefront.
pub fn routes() -> Router<AppState> {
    Router::new()
        // Auth routes
        .nest("/auth", auth_routes())
        // `WebAuthn` API
        .nest("/api/auth/webauthn", webauthn_api_routes())
}

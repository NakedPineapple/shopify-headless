//! HTTP route handlers for admin.
//!
//! # Route Structure
//!
//! ```text
//! GET  /                        - Dashboard (auth required)
//! GET  /health                 - Health check
//!
//! # Authentication
//! GET  /auth/login             - Login page (passkey only)
//! POST /auth/logout            - Logout
//! GET  /auth/setup             - New admin setup page
//!
//! # Setup API (for new admin registration)
//! POST /api/auth/setup/send-code       - Send verification code to email
//! POST /api/auth/setup/verify-code     - Verify the code
//! POST /api/auth/setup/register/start  - Start passkey registration
//! POST /api/auth/setup/register/finish - Finish registration and create user
//!
//! # WebAuthn API (for existing users)
//! POST /api/auth/webauthn/authenticate/start  - Start passkey login
//! POST /api/auth/webauthn/authenticate/finish - Finish passkey login
//! POST /api/auth/webauthn/register/start      - Start passkey registration (auth required)
//! POST /api/auth/webauthn/register/finish     - Finish passkey registration (auth required)
//!
//! # Shopify OAuth (super_admin only)
//! GET  /shopify                - Shopify settings page
//! GET  /shopify/connect        - Start OAuth flow
//! GET  /shopify/callback       - OAuth callback
//! GET  /shopify/disconnect     - Disconnect from Shopify
//!
//! # Products (auth required)
//! GET  /products               - Products list
//!
//! # Orders (auth required)
//! GET  /orders                 - Orders list
//!
//! # Customers (auth required)
//! GET  /customers              - Customers list
//!
//! # Chat (Claude AI) - auth required
//! GET  /chat/sessions          - List chat sessions
//! POST /chat/sessions          - Create new chat session
//! GET  /chat/sessions/:id      - Get chat session with messages
//! POST /chat/sessions/:id/messages - Send message (returns response)
//! ```

pub mod admin_users;
pub mod api;
pub mod auth;
pub mod chat;
pub mod collections;
pub mod customers;
pub mod dashboard;
pub mod discounts;
pub mod gift_cards;
pub mod inventory;
pub mod orders;
pub mod payouts;
pub mod products;
pub mod setup;
pub mod shopify;

use axum::{
    Router,
    routing::{get, post},
};

use crate::state::AppState;

/// Build the complete router for the admin application.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(dashboard::dashboard))
        // Products CRUD
        .route("/products", get(products::index).post(products::create))
        .route("/products/new", get(products::new_product))
        .route("/products/{id}", get(products::show).post(products::update))
        .route("/products/{id}/edit", get(products::edit))
        .route("/products/{id}/archive", post(products::archive))
        .route("/products/{id}/delete", post(products::delete))
        .route(
            "/products/{id}/variants/{variant_id}",
            post(products::update_variant),
        )
        .route(
            "/products/{id}/images/{media_id}",
            axum::routing::delete(products::delete_image),
        )
        // Orders CRUD
        .route("/orders", get(orders::index))
        .route("/orders/{id}", get(orders::show))
        .route("/orders/{id}/note", post(orders::update_note))
        .route("/orders/{id}/mark-paid", post(orders::mark_paid))
        .route("/orders/{id}/cancel", post(orders::cancel))
        // Customers
        .route("/customers", get(customers::index))
        // Collections CRUD
        .route(
            "/collections",
            get(collections::index).post(collections::create),
        )
        .route("/collections/new", get(collections::new_collection))
        .route("/collections/{id}/edit", get(collections::edit))
        .route("/collections/{id}", post(collections::update))
        .route("/collections/{id}/delete", post(collections::delete))
        // Discounts CRUD
        .route("/discounts", get(discounts::index).post(discounts::create))
        .route("/discounts/new", get(discounts::new_discount))
        .route("/discounts/{id}/edit", get(discounts::edit))
        .route("/discounts/{id}", post(discounts::update))
        .route("/discounts/{id}/deactivate", post(discounts::deactivate))
        // Inventory management
        .route("/inventory", get(inventory::index))
        .route("/inventory/adjust", post(inventory::adjust))
        .route("/inventory/set", post(inventory::set))
        // Gift Cards CRUD
        .route(
            "/gift-cards",
            get(gift_cards::index).post(gift_cards::create),
        )
        .route("/gift-cards/new", get(gift_cards::new_gift_card))
        .route("/gift-cards/{id}/disable", post(gift_cards::disable))
        // Payouts (read-only)
        .route("/payouts", get(payouts::index))
        .route("/payouts/{id}", get(payouts::show))
        // Admin management (super_admin only)
        .route("/admin-users", get(admin_users::index))
        // Auth
        .merge(auth::router())
        .merge(setup::router())
        .merge(api::router())
        .merge(chat::router())
        // Shopify OAuth
        .merge(shopify::router())
}

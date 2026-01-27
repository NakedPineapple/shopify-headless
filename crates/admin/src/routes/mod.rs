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

pub mod api;
pub mod auth;
pub mod chat;
pub mod customers;
pub mod dashboard;
pub mod orders;
pub mod products;
pub mod setup;
pub mod shopify;

use axum::{Router, routing::get};

use crate::state::AppState;

/// Build the complete router for the admin application.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(dashboard::dashboard))
        // List views
        .route("/products", get(products::index))
        .route("/orders", get(orders::index))
        .route("/customers", get(customers::index))
        // Auth
        .merge(auth::router())
        .merge(setup::router())
        .merge(api::router())
        .merge(chat::router())
        // Shopify OAuth
        .merge(shopify::router())
}

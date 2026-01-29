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
pub mod analytics;
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
pub mod settings;
pub mod setup;
pub mod shopify;

use axum::{
    Router,
    routing::{get, post},
};

use crate::state::AppState;

/// Build product routes.
fn product_routes() -> Router<AppState> {
    Router::new()
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
        .route("/products/{id}/images", post(products::upload_image))
        .route(
            "/products/{id}/images/{media_id}",
            axum::routing::delete(products::delete_image),
        )
        .route(
            "/products/{id}/images/reorder",
            post(products::reorder_images),
        )
        .route(
            "/products/{id}/images/{media_id}/alt",
            post(products::update_image_alt),
        )
}

/// Build customer routes.
fn customer_routes() -> Router<AppState> {
    Router::new()
        .route("/customers", get(customers::index).post(customers::create))
        .route("/customers/new", get(customers::new))
        .route(
            "/customers/{id}",
            get(customers::show).post(customers::update),
        )
        .route("/customers/{id}/edit", get(customers::edit))
        .route("/customers/{id}/delete", post(customers::delete))
        .route("/customers/{id}/tags", post(customers::update_tags))
        .route("/customers/{id}/note", post(customers::update_note))
        .route(
            "/customers/{id}/marketing",
            post(customers::update_marketing),
        )
        .route("/customers/{id}/send-invite", post(customers::send_invite))
        .route(
            "/customers/{id}/activation-url",
            post(customers::activation_url),
        )
        .route("/customers/{id}/addresses", post(customers::address_create))
        .route(
            "/customers/{id}/addresses/{address_id}",
            post(customers::address_update).delete(customers::address_delete),
        )
        .route(
            "/customers/{id}/addresses/{address_id}/default",
            post(customers::set_default_address),
        )
        .route("/customers/{id}/merge", post(customers::merge))
        .route("/customers/bulk/tags", post(customers::bulk_tags))
        .route("/customers/bulk/marketing", post(customers::bulk_marketing))
}

/// Build collection routes.
fn collection_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/collections",
            get(collections::index).post(collections::create),
        )
        .route("/collections/new", get(collections::new_collection))
        .route(
            "/collections/{id}",
            get(collections::show).post(collections::update),
        )
        .route("/collections/{id}/edit", get(collections::edit))
        .route("/collections/{id}/delete", post(collections::delete))
        .route(
            "/collections/{id}/products",
            post(collections::add_products),
        )
        .route(
            "/collections/{id}/products/remove",
            post(collections::remove_products),
        )
        .route(
            "/collections/{id}/products/reorder",
            post(collections::reorder_products),
        )
        .route(
            "/collections/{id}/sort-order",
            post(collections::update_sort_order),
        )
        .route("/collections/{id}/image", post(collections::upload_image))
        .route(
            "/collections/{id}/image/delete",
            post(collections::delete_image),
        )
}

/// Build order routes.
fn order_routes() -> Router<AppState> {
    Router::new()
        .route("/orders", get(orders::index))
        .route("/orders/{id}", get(orders::show))
        .route("/orders/{id}/note", post(orders::update_note))
        .route("/orders/{id}/mark-paid", post(orders::mark_paid))
        .route("/orders/{id}/cancel", post(orders::cancel))
        .route("/orders/{id}/tags", post(orders::update_tags))
        .route("/orders/{id}/fulfill", post(orders::fulfill))
        .route(
            "/orders/{order_id}/fulfillment-orders/{fo_id}/hold",
            post(orders::hold_fulfillment),
        )
        .route(
            "/orders/{order_id}/fulfillment-orders/{fo_id}/release",
            post(orders::release_hold),
        )
        .route(
            "/orders/{id}/calculate-refund",
            get(orders::calculate_refund),
        )
        .route("/orders/{id}/refund", post(orders::refund))
        .route("/orders/{id}/return", post(orders::create_return))
        .route("/orders/{id}/capture", post(orders::capture))
        .route("/orders/{id}/archive", post(orders::archive))
        .route("/orders/{id}/print", get(orders::print))
        .route(
            "/orders/{id}/edit",
            get(orders::edit).post(orders::edit_commit),
        )
        .route(
            "/orders/{id}/edit/add-variant",
            post(orders::edit_add_variant),
        )
        .route(
            "/orders/{id}/edit/add-custom-item",
            post(orders::edit_add_custom_item),
        )
        .route(
            "/orders/{id}/edit/set-quantity",
            post(orders::edit_set_quantity),
        )
        .route(
            "/orders/{id}/edit/add-discount",
            post(orders::edit_add_discount),
        )
        .route(
            "/orders/{id}/edit/update-discount",
            post(orders::edit_update_discount),
        )
        .route(
            "/orders/{id}/edit/remove-discount",
            post(orders::edit_remove_discount),
        )
        .route(
            "/orders/{id}/edit/add-shipping",
            post(orders::edit_add_shipping),
        )
        .route(
            "/orders/{id}/edit/update-shipping",
            post(orders::edit_update_shipping),
        )
        .route(
            "/orders/{id}/edit/remove-shipping",
            post(orders::edit_remove_shipping),
        )
        .route("/orders/{id}/edit/discard", post(orders::edit_discard))
        .route(
            "/orders/{id}/edit/search-products",
            get(orders::edit_search_products),
        )
        .route("/orders/bulk/add-tags", post(orders::bulk_add_tags))
        .route("/orders/bulk/remove-tags", post(orders::bulk_remove_tags))
        .route("/orders/bulk/archive", post(orders::bulk_archive))
        .route("/orders/bulk/cancel", post(orders::bulk_cancel))
}

/// Build admin user management routes.
fn admin_user_routes() -> Router<AppState> {
    Router::new()
        .route("/admin-users", get(admin_users::index))
        .route("/admin-users/{id}/role", post(admin_users::update_role))
        .route("/admin-users/{id}/delete", post(admin_users::delete_user))
        .route("/admin-users/invites", post(admin_users::create_invite))
        .route(
            "/admin-users/invites/{id}/delete",
            post(admin_users::delete_invite),
        )
}

/// Build the complete router for the admin application.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(dashboard::dashboard))
        .merge(product_routes())
        .merge(order_routes())
        .merge(customer_routes())
        .merge(collection_routes())
        // Discounts CRUD
        .route("/discounts", get(discounts::index).post(discounts::create))
        .route("/discounts/new", get(discounts::new_step1))
        .route("/discounts/new/{method}", get(discounts::new_step2))
        .route("/discounts/new/{method}/{type}", get(discounts::new_step3))
        .route(
            "/discounts/{id}",
            get(discounts::show).post(discounts::update),
        )
        .route("/discounts/{id}/edit", get(discounts::edit))
        .route("/discounts/{id}/activate", post(discounts::activate))
        .route("/discounts/{id}/deactivate", post(discounts::deactivate))
        .route("/discounts/{id}/delete", post(discounts::delete))
        .route("/discounts/{id}/duplicate", post(discounts::duplicate))
        // Discount bulk actions
        .route("/discounts/bulk/activate", post(discounts::bulk_activate))
        .route(
            "/discounts/bulk/deactivate",
            post(discounts::bulk_deactivate),
        )
        .route("/discounts/bulk/delete", post(discounts::bulk_delete))
        // Discount API endpoints
        .route("/api/products/search", get(discounts::api_search_products))
        .route(
            "/api/collections/search",
            get(discounts::api_search_collections),
        )
        .route(
            "/api/customers/search",
            get(discounts::api_search_customers),
        )
        .route(
            "/api/customer-segments",
            get(discounts::api_customer_segments),
        )
        .route("/discounts/{id}/codes", post(discounts::api_add_codes))
        // Inventory management
        .route("/inventory", get(inventory::index))
        .route("/inventory/adjust", post(inventory::adjust))
        .route("/inventory/set", post(inventory::set))
        .route("/inventory/move", post(inventory::move_quantity))
        .route(
            "/inventory/{id}",
            get(inventory::show).post(inventory::update),
        )
        .route("/inventory/{id}/edit", get(inventory::edit))
        .route("/inventory/{id}/activate", post(inventory::activate))
        .route("/inventory/{id}/deactivate", post(inventory::deactivate))
        // Gift Cards CRUD
        .route(
            "/gift-cards",
            get(gift_cards::index).post(gift_cards::create),
        )
        .route("/gift-cards/new", get(gift_cards::new_gift_card))
        .route(
            "/gift-cards/{id}",
            get(gift_cards::show).post(gift_cards::update),
        )
        .route("/gift-cards/{id}/edit", get(gift_cards::edit))
        .route("/gift-cards/{id}/deactivate", post(gift_cards::deactivate))
        .route(
            "/gift-cards/{id}/adjust-balance",
            post(gift_cards::adjust_balance),
        )
        .route(
            "/gift-cards/{id}/notify-customer",
            post(gift_cards::notify_customer),
        )
        .route(
            "/gift-cards/{id}/notify-recipient",
            post(gift_cards::notify_recipient),
        )
        .route("/gift-cards/{id}/note", post(gift_cards::update_note))
        .route(
            "/gift-cards/bulk/deactivate",
            post(gift_cards::bulk_deactivate),
        )
        // Analytics
        .route("/analytics", get(analytics::index))
        .route("/analytics/channels", get(analytics::channels))
        .route("/analytics/channels/{name}", get(analytics::channel_detail))
        // Payouts
        .route("/payouts", get(payouts::index))
        .route("/payouts/disputes", get(payouts::disputes_index))
        .route(
            "/payouts/disputes/{id}",
            get(payouts::dispute_show).post(payouts::dispute_submit_evidence),
        )
        .route("/payouts/bank-accounts", get(payouts::bank_accounts))
        .route("/payouts/settings", get(payouts::settings))
        .route("/payouts/{id}", get(payouts::show))
        .route("/payouts/{id}/transactions", get(payouts::transactions))
        .route("/payouts/{id}/export", get(payouts::export_csv))
        // Admin management (super_admin only)
        .merge(admin_user_routes())
        // Auth
        .merge(auth::router())
        .merge(setup::router())
        .merge(api::router())
        .merge(chat::router())
        // Shopify OAuth
        .merge(shopify::router())
        // Settings
        .merge(settings::router())
}

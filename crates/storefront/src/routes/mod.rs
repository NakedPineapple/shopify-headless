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
//! GET  /login                  - Login page
//! POST /login                  - Login action
//! GET  /register               - Register page
//! POST /register               - Register action
//! POST /logout                 - Logout action
//! GET  /forgot-password        - Forgot password page
//! POST /forgot-password        - Send reset email
//! GET  /reset-password/:token  - Reset password page
//! POST /reset-password/:token  - Reset password action
//! GET  /verify-email/:code     - Verify email
//!
//! # Account (requires auth)
//! GET  /account                - Account overview
//! GET  /account/orders         - Order history
//! GET  /account/orders/:id     - Order detail
//! GET  /account/addresses      - Address management
//! POST /account/addresses      - Add address
//! PUT  /account/addresses/:id  - Update address
//! DELETE /account/addresses/:id - Delete address
//! GET  /account/passkeys       - Passkey management
//! POST /account/passkeys       - Register passkey
//!
//! # Content
//! GET  /blog                   - Blog listing
//! GET  /blog/:slug             - Blog post
//! GET  /pages/:slug            - Static pages (terms, privacy, etc.)
//! ```
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use axum::{routing::{get, post, put, delete}, Router};
//! use crate::state::AppState;
//!
//! pub mod account;
//! pub mod auth;
//! pub mod blog;
//! pub mod cart;
//! pub mod checkout;
//! pub mod collections;
//! pub mod health;
//! pub mod home;
//! pub mod pages;
//! pub mod products;
//!
//! pub fn routes() -> Router<AppState> {
//!     Router::new()
//!         // Health
//!         .route("/health", get(health::health))
//!
//!         // Home
//!         .route("/", get(home::index))
//!
//!         // Products
//!         .route("/products", get(products::index))
//!         .route("/products/:handle", get(products::show))
//!
//!         // Collections
//!         .route("/collections", get(collections::index))
//!         .route("/collections/:handle", get(collections::show))
//!
//!         // Cart
//!         .route("/cart", get(cart::index))
//!         .route("/cart/add", post(cart::add))
//!         .route("/cart/update", post(cart::update))
//!         .route("/cart/remove", post(cart::remove))
//!         .route("/cart/count", get(cart::count))
//!
//!         // Checkout
//!         .route("/checkout", get(checkout::redirect))
//!
//!         // Auth
//!         .route("/login", get(auth::login_page).post(auth::login))
//!         .route("/register", get(auth::register_page).post(auth::register))
//!         .route("/logout", post(auth::logout))
//!         .route("/forgot-password", get(auth::forgot_password_page).post(auth::forgot_password))
//!         .route("/reset-password/:token", get(auth::reset_password_page).post(auth::reset_password))
//!         .route("/verify-email/:code", get(auth::verify_email))
//!
//!         // Account (protected)
//!         .route("/account", get(account::index))
//!         .route("/account/orders", get(account::orders))
//!         .route("/account/orders/:id", get(account::order_detail))
//!         .route("/account/addresses", get(account::addresses).post(account::create_address))
//!         .route("/account/addresses/:id", put(account::update_address).delete(account::delete_address))
//!         .route("/account/passkeys", get(account::passkeys).post(account::register_passkey))
//!
//!         // Content
//!         .route("/blog", get(blog::index))
//!         .route("/blog/:slug", get(blog::show))
//!         .route("/pages/:slug", get(pages::show))
//! }
//! ```

// TODO: Implement route handlers

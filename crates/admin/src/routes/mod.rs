//! HTTP route handlers for admin.
//!
//! # Route Structure
//!
//! ```text
//! GET  /health                 - Health check
//!
//! # Dashboard
//! GET  /                       - Dashboard overview
//!
//! # Auth (WebAuthn only - no passwords in admin)
//! GET  /login                  - Login page
//! POST /login/start            - Start WebAuthn authentication
//! POST /login/finish           - Finish WebAuthn authentication
//! POST /logout                 - Logout
//!
//! # Products (read from Shopify)
//! GET  /products               - Product listing
//! GET  /products/:id           - Product detail
//!
//! # Orders (read from Shopify)
//! GET  /orders                 - Order listing
//! GET  /orders/:id             - Order detail
//!
//! # Customers (read from Shopify)
//! GET  /customers              - Customer listing
//! GET  /customers/:id          - Customer detail
//!
//! # Inventory (read/write to Shopify)
//! GET  /inventory              - Inventory overview
//! POST /inventory/adjust       - Adjust inventory levels
//!
//! # Chat (Claude AI)
//! GET  /chat                   - Chat interface
//! GET  /chat/sessions          - List chat sessions
//! POST /chat/sessions          - Create new chat session
//! GET  /chat/sessions/:id      - Get chat session with messages
//! POST /chat/sessions/:id/messages - Send message (returns streamed response)
//!
//! # Settings
//! GET  /settings               - Settings page
//! POST /settings               - Update settings
//!
//! # Admin Users (super admin only)
//! GET  /admin-users            - List admin users
//! POST /admin-users            - Create admin user
//! DELETE /admin-users/:id      - Remove admin user
//! ```
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use axum::{routing::{get, post, delete}, Router};
//! use crate::state::AppState;
//!
//! pub mod auth;
//! pub mod chat;
//! pub mod customers;
//! pub mod dashboard;
//! pub mod health;
//! pub mod inventory;
//! pub mod orders;
//! pub mod products;
//! pub mod settings;
//! pub mod users;
//!
//! pub fn routes() -> Router<AppState> {
//!     Router::new()
//!         // Health
//!         .route("/health", get(health::health))
//!
//!         // Dashboard
//!         .route("/", get(dashboard::index))
//!
//!         // Auth
//!         .route("/login", get(auth::login_page))
//!         .route("/login/start", post(auth::login_start))
//!         .route("/login/finish", post(auth::login_finish))
//!         .route("/logout", post(auth::logout))
//!
//!         // Products
//!         .route("/products", get(products::index))
//!         .route("/products/:id", get(products::show))
//!
//!         // Orders
//!         .route("/orders", get(orders::index))
//!         .route("/orders/:id", get(orders::show))
//!
//!         // Customers
//!         .route("/customers", get(customers::index))
//!         .route("/customers/:id", get(customers::show))
//!
//!         // Inventory
//!         .route("/inventory", get(inventory::index))
//!         .route("/inventory/adjust", post(inventory::adjust))
//!
//!         // Chat
//!         .route("/chat", get(chat::index))
//!         .route("/chat/sessions", get(chat::list_sessions).post(chat::create_session))
//!         .route("/chat/sessions/:id", get(chat::get_session))
//!         .route("/chat/sessions/:id/messages", post(chat::send_message))
//!
//!         // Settings
//!         .route("/settings", get(settings::index).post(settings::update))
//!
//!         // Admin Users
//!         .route("/admin-users", get(users::index).post(users::create))
//!         .route("/admin-users/:id", delete(users::delete))
//! }
//! ```

// TODO: Implement route handlers

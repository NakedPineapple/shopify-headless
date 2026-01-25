//! HTTP route handlers for admin.
//!
//! # Route Structure
//!
//! ```text
//! GET  /health                 - Health check
//!
//! # Authentication
//! GET  /auth/login             - Login page (passkey only)
//! POST /auth/logout            - Logout
//!
//! # WebAuthn API
//! POST /api/auth/webauthn/authenticate/start  - Start passkey login
//! POST /api/auth/webauthn/authenticate/finish - Finish passkey login
//! POST /api/auth/webauthn/register/start      - Start passkey registration (auth required)
//! POST /api/auth/webauthn/register/finish     - Finish passkey registration (auth required)
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

use axum::Router;

use crate::state::AppState;

/// Build the complete router for the admin application.
pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(api::router())
        .merge(chat::router())
}

//! HTTP route handlers for admin.
//!
//! # Route Structure
//!
//! ```text
//! GET  /health                 - Health check
//!
//! # Chat (Claude AI)
//! GET  /chat/sessions          - List chat sessions
//! POST /chat/sessions          - Create new chat session
//! GET  /chat/sessions/:id      - Get chat session with messages
//! POST /chat/sessions/:id/messages - Send message (returns response)
//! ```

pub mod chat;

use axum::Router;

use crate::state::AppState;

/// Build the complete router for the admin application.
pub fn routes() -> Router<AppState> {
    Router::new().merge(chat::router())
}

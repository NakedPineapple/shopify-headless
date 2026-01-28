//! API route handlers for admin.
//!
//! JSON API endpoints for various admin operations.

pub mod preferences;
pub mod webauthn;

use axum::Router;

use crate::state::AppState;

/// Build the complete API router.
pub fn router() -> Router<AppState> {
    Router::new()
        .merge(webauthn::router())
        .merge(preferences::router())
}

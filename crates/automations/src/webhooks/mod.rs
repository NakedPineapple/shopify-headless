//! Public webhook ingestion layer.
//!
//! Receives webhooks from external services (Shopify, GitHub, Sentry, Fly.io,
//! Better Stack), verifies signatures, and stores events in `admin.webhook_event`
//! for asynchronous processing by the scheduler.
//!
//! # Security
//!
//! This module runs on a separate HTTP listener from internal endpoints and uses
//! a restricted [`WebhookState`] with a limited-privilege database connection.
//! Webhook handlers have no access to the Shopify Admin API token, service
//! credentials, or any table other than `admin.webhook_event`.

mod betterstack;
pub mod db;
mod fly;
mod github;
mod sentry_hook;
mod shopify;
pub mod state;
mod verify;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::post;

use state::WebhookState;

/// Maximum request body size for webhook payloads (1 MB).
const MAX_BODY_SIZE: usize = 1_048_576;

/// Build the public webhook router.
///
/// All routes verify source-specific signatures before any database access.
pub fn router(state: WebhookState) -> Router {
    Router::new()
        .route("/webhooks/shopify/{*topic}", post(shopify::handle))
        .route("/webhooks/github", post(github::handle))
        .route("/webhooks/sentry", post(sentry_hook::handle))
        .route("/webhooks/fly", post(fly::handle))
        .route("/webhooks/betterstack", post(betterstack::handle))
        .layer(DefaultBodyLimit::max(MAX_BODY_SIZE))
        .with_state(state)
}

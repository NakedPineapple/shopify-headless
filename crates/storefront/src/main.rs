//! Naked Pineapple Storefront - Public e-commerce site.
//!
//! This binary serves the public-facing storefront on port 3000.
//!
//! # Architecture
//!
//! - Axum web framework with HTMX for interactivity
//! - Askama templates for server-side rendering
//! - Shopify Storefront API for products, collections, and cart
//! - Shopify Customer Account API for authentication
//! - `PostgreSQL` for local user data (separate from Shopify)
//!
//! # Security
//!
//! This binary only has access to:
//! - Shopify Storefront API (public access)
//! - Shopify Customer Account API (OAuth)
//! - Local `PostgreSQL` database (`np_storefront`)
//!
//! It does NOT have access to:
//! - Shopify Admin API (that's in the admin binary)
//! - Admin `PostgreSQL` database (`np_admin`)

#![cfg_attr(not(test), forbid(unsafe_code))]
// Allow dead code during incremental development - many features are not yet wired up
#![allow(dead_code)]
#![allow(unused_imports)]

use std::path::Path;

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Router, routing::get};
use tower_http::services::ServeDir;

mod config;
mod content;
mod db;
mod error;
mod filters;
mod middleware;
mod models;
mod routes;
mod search;
mod services;
mod shopify;
mod state;

use config::StorefrontConfig;
use sentry::integrations::tracing as sentry_tracing;
use state::AppState;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize Sentry error tracking and return guard that must be kept alive.
fn init_sentry(config: &StorefrontConfig) -> Option<sentry::ClientInitGuard> {
    let dsn = config.sentry_dsn.as_ref()?;

    let guard = sentry::init((
        dsn.as_str(),
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: config
                .sentry_environment
                .clone()
                .map(std::borrow::Cow::Owned),
            sample_rate: config.sentry_sample_rate,
            traces_sample_rate: config.sentry_traces_sample_rate,
            attach_stacktrace: true,
            ..Default::default()
        },
    ));

    tracing::info!("Sentry initialized");
    Some(guard)
}

/// Filter tracing events to Sentry event types.
fn sentry_event_filter(metadata: &tracing::Metadata<'_>) -> sentry_tracing::EventFilter {
    match *metadata.level() {
        tracing::Level::ERROR | tracing::Level::WARN => sentry_tracing::EventFilter::Event,
        tracing::Level::INFO | tracing::Level::DEBUG => sentry_tracing::EventFilter::Breadcrumb,
        _ => sentry_tracing::EventFilter::Ignore,
    }
}

#[tokio::main]
async fn main() {
    // Load configuration from environment (needed for Sentry init)
    let config = StorefrontConfig::from_env().expect("Failed to load configuration");

    // Initialize Sentry (must be done before tracing subscriber)
    let _sentry_guard = init_sentry(&config);

    // Initialize tracing with EnvFilter and Sentry integration
    // Defaults to info level for our crate if RUST_LOG is not set
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "naked_pineapple_storefront=info,tower_http=debug".into());

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .with(sentry_tracing::layer().event_filter(sentry_event_filter))
        .init();

    // Initialize database connection pool
    let pool = db::create_pool(&config.database_url)
        .await
        .expect("Failed to create database pool");
    tracing::info!("Database pool created");

    // NOTE: Migrations are NOT run automatically on startup.
    // Run them explicitly via: cargo run -p naked-pineapple-cli -- migrate storefront

    // TODO: Initialize Shopify clients
    // let storefront_client = shopify::StorefrontClient::new(&config.shopify);
    // let customer_client = shopify::CustomerClient::new(&config.shopify);

    // Build application state
    // Content is loaded from the storefront crate's `content/` directory
    let content_dir = Path::new("crates/storefront/content");
    let state = AppState::new(config.clone(), pool, content_dir)
        .expect("Failed to initialize application state");

    // Start building search index in background
    state.start_search_indexing();
    tracing::info!("Search index build started (async)");

    // Create session layer
    let session_layer = middleware::create_session_layer(state.pool(), state.config());

    // Build router
    let app = Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(readiness))
        .merge(routes::routes())
        .nest_service("/static", ServeDir::new("crates/storefront/static"))
        .layer(session_layer)
        .with_state(state)
        // Sentry layers (outermost for full request coverage)
        .layer(sentry_tower::NewSentryLayer::new_from_top())
        .layer(sentry_tower::SentryHttpLayer::new().enable_transaction());

    // Start server
    let addr = config.socket_addr();
    tracing::info!("storefront listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");
}

/// Liveness health check endpoint.
///
/// Returns "ok" if the server is running. Does not check dependencies.
async fn health() -> &'static str {
    "ok"
}

/// Readiness health check endpoint.
///
/// Verifies database connectivity before returning OK.
/// Returns 503 Service Unavailable if the database is not reachable.
async fn readiness(State(state): State<AppState>) -> StatusCode {
    match sqlx::query("SELECT 1").fetch_one(state.pool()).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM).
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown");
}

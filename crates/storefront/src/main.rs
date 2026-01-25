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

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Router, routing::get};

mod config;
mod db;
mod error;
mod middleware;
mod models;
mod routes;
mod services;
mod shopify;
mod state;

use config::StorefrontConfig;
use state::AppState;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration from environment
    let config = StorefrontConfig::from_env().expect("Failed to load configuration");

    // TODO: Initialize Sentry for error tracking
    // let _guard = sentry::init(sentry::ClientOptions {
    //     dsn: config.sentry_dsn.clone(),
    //     release: sentry::release_name!(),
    //     ..Default::default()
    // });

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
    let state = AppState::new(config.clone(), pool);

    // Build router
    let app = Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(readiness))
        // TODO: Add routes
        // .merge(routes::routes())
        // TODO: Add middleware stack
        // .layer(middleware::stack())
        .with_state(state);

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

//! Naked Pineapple Admin - Internal administration panel.
//!
//! This binary serves the admin panel on port 3001.
//!
//! # Security
//!
//! **CRITICAL: This binary must ONLY run on Tailscale-protected infrastructure.**
//!
//! - Accessible only via Tailscale VPN
//! - Requires MDM-managed devices
//! - Contains HIGH PRIVILEGE Shopify Admin API token
//! - Has access to admin-only `PostgreSQL` database (`np_admin`)
//!
//! # Architecture
//!
//! - Axum web framework
//! - Askama templates for server-side rendering
//! - Shopify Admin API for full store management
//! - Claude API for AI-powered chat assistant
//! - `PostgreSQL` for admin users and chat history
//!
//! # APIs
//!
//! - Shopify Admin API (HIGH PRIVILEGE)
//! - Claude API (for AI chat features)

#![cfg_attr(not(test), forbid(unsafe_code))]

use axum::{Router, routing::get};
use std::net::SocketAddr;

mod claude;
mod config;
mod db;
mod error;
mod middleware;
mod models;
mod routes;
mod services;
mod shopify;
mod state;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // TODO: Load configuration from environment
    // let config = config::AdminConfig::from_env()
    //     .expect("Failed to load configuration");

    // TODO: Initialize Sentry for error tracking
    // let _guard = sentry::init(sentry::ClientOptions {
    //     dsn: config.sentry_dsn.clone(),
    //     release: sentry::release_name!(),
    //     ..Default::default()
    // });

    // TODO: Initialize database connection pool
    // let pool = sqlx::PgPool::connect(&config.database_url)
    //     .await
    //     .expect("Failed to connect to database");

    // TODO: Run migrations
    // sqlx::migrate!("./migrations")
    //     .run(&pool)
    //     .await
    //     .expect("Failed to run migrations");

    // TODO: Initialize clients
    // let shopify_client = shopify::AdminClient::new(&config.shopify);
    // let claude_client = claude::ClaudeClient::new(&config.claude);

    // TODO: Build application state
    // let state = state::AppState::new(config, pool, shopify_client, claude_client);

    // Build router
    let app = Router::new()
        .route("/health", get(health))
        // TODO: Add routes
        // .merge(routes::routes())
        // TODO: Add middleware stack
        // .layer(middleware::stack())
        // .with_state(state)
        ;

    // Start server
    // NOTE: Binding to 127.0.0.1 - Tailscale handles external access
    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));
    tracing::info!("admin listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app).await.expect("Server error");
}

/// Health check endpoint.
async fn health() -> &'static str {
    "ok"
}

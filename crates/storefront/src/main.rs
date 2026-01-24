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

use axum::{Router, routing::get};
use std::net::SocketAddr;

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
    // let config = config::StorefrontConfig::from_env()
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

    // TODO: Initialize Shopify clients
    // let storefront_client = shopify::StorefrontClient::new(&config.shopify);
    // let customer_client = shopify::CustomerClient::new(&config.shopify);

    // TODO: Build application state
    // let state = state::AppState::new(config, pool, storefront_client, customer_client);

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
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("storefront listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app).await.expect("Server error");
}

/// Health check endpoint.
async fn health() -> &'static str {
    "ok"
}

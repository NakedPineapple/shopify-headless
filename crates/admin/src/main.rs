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
// Allow dead code during incremental development - many features are not yet wired up
#![allow(dead_code)]
#![allow(unused_imports)]

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Router, routing::get};
use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use secrecy::ExposeSecret;
use tower_http::services::ServeDir;

mod claude;
mod config;
mod db;
mod error;
mod filters;
mod middleware;
mod models;
mod routes;
mod services;
mod shopify;
mod state;

use config::AdminConfig;
use middleware::create_session_layer;
use state::AppState;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Install rustls crypto provider (must be done before any TLS operations)
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Initialize tracing with EnvFilter
    // Defaults to info level for our crate if RUST_LOG is not set
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "naked_pineapple_admin=info,tower_http=debug".into());

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration from environment
    let config = AdminConfig::from_env().expect("Failed to load configuration");

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
    // Run them explicitly via: cargo run -p naked-pineapple-cli -- migrate admin

    // Create session layer (PostgreSQL-backed with SameSite=Strict)
    let session_layer = create_session_layer(&pool, &config);

    // Build application state (includes WebAuthn)
    let state = AppState::new(config.clone(), pool).expect("Failed to create application state");

    // Build router
    let app = Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(readiness))
        .merge(routes::routes())
        .nest_service("/static", ServeDir::new("crates/admin/static"))
        .layer(session_layer)
        .with_state(state);

    // Start server
    let addr = config.socket_addr();

    if let Some(tls_config) = &config.tls {
        let rustls_config = RustlsConfig::from_pem(
            tls_config.cert_pem.as_bytes().to_vec(),
            tls_config.key_pem.expose_secret().as_bytes().to_vec(),
        )
        .await
        .expect("Failed to load TLS certificates");

        tracing::info!("admin listening on https://{}", addr);

        let handle = Handle::new();
        let shutdown_handle = handle.clone();

        // Spawn task to handle graceful shutdown
        tokio::spawn(async move {
            shutdown_signal().await;
            shutdown_handle.graceful_shutdown(Some(std::time::Duration::from_secs(30)));
        });

        axum_server::bind_rustls(addr, rustls_config)
            .handle(handle)
            .serve(app.into_make_service())
            .await
            .expect("Server error");
    } else {
        // NOTE: Binding to 127.0.0.1 - Tailscale handles external access
        tracing::info!("admin listening on http://{}", addr);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("Failed to bind to address");

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .expect("Server error");
    }
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

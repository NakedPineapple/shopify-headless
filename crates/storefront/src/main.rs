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

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, HeaderValue};
use axum::middleware::from_fn;
use axum::{Router, routing::get};
use axum_prometheus::PrometheusMetricLayer;
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

/// Sets cache-control header only on successful (2xx) responses.
/// This prevents Cloudflare from caching 404s with immutable headers.
fn cache_on_success<B>(
    header_value: HeaderValue,
) -> impl Fn(axum::http::Response<B>) -> axum::http::Response<B> + Clone {
    move |mut response: axum::http::Response<B>| {
        if response.status().is_success() {
            response
                .headers_mut()
                .insert(CACHE_CONTROL, header_value.clone());
        }
        response
    }
}

mod config;
mod content;
mod db;
mod error;
mod filters;
mod image_manifest;
mod logging;
mod middleware;
mod models;
mod routes;
mod search;
mod services;
mod shopify;
mod state;

use config::StorefrontConfig;
use state::AppState;

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

/// Handle panics by logging and returning a 500 response.
#[allow(clippy::needless_pass_by_value)] // CatchPanicLayer requires ownership
fn handle_panic(
    panic_info: Box<dyn std::any::Any + Send>,
) -> axum::http::Response<axum::body::Body> {
    let details = panic_info
        .downcast_ref::<&str>()
        .map(ToString::to_string)
        .or_else(|| panic_info.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "Unknown panic".to_string());
    tracing::error!(panic = %details, "Request handler panicked");
    axum::http::Response::builder()
        .status(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        .body(axum::body::Body::from("Internal Server Error"))
        .expect("building a simple 500 response should never fail")
}

/// Build the static file service routes with appropriate cache headers.
fn build_static_routes() -> Router<AppState> {
    // Cache control headers for static assets
    // Optimized images - immutable (filenames include size suffix, content never changes)
    let cache_immutable = HeaderValue::from_static("public, max-age=31536000, immutable");
    // Vendor libraries - long cache (versioned, rarely change)
    let cache_long = HeaderValue::from_static("public, max-age=2592000");
    // CSS/JS - short cache (may change between deploys)
    let cache_short = HeaderValue::from_static("public, max-age=86400, must-revalidate");

    Router::new()
        // Optimized images - immutable (1 year cache, only on success)
        .nest_service(
            "/static/images/derived",
            ServiceBuilder::new()
                .map_response(cache_on_success(cache_immutable))
                .service(ServeDir::new("crates/storefront/static/images/derived")),
        )
        // Original images fallback (for development) - short cache
        .nest_service(
            "/static/images/original",
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    CACHE_CONTROL,
                    cache_short.clone(),
                ))
                .service(ServeDir::new("crates/storefront/static/images/original")),
        )
        // Vendor libraries (htmx, swiper, fonts) - long cache
        .nest_service(
            "/static/vendor",
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    CACHE_CONTROL,
                    cache_long,
                ))
                .service(ServeDir::new("crates/storefront/static/vendor")),
        )
        // CSS - short cache with revalidation
        .nest_service(
            "/static/css",
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    CACHE_CONTROL,
                    cache_short.clone(),
                ))
                .service(ServeDir::new("crates/storefront/static/css")),
        )
        // JS - short cache with revalidation
        .nest_service(
            "/static/js",
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    CACHE_CONTROL,
                    cache_short.clone(),
                ))
                .service(ServeDir::new("crates/storefront/static/js")),
        )
        // Fallback for any other static files
        .nest_service(
            "/static",
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    CACHE_CONTROL,
                    cache_short,
                ))
                .service(ServeDir::new("crates/storefront/static")),
        )
}

#[tokio::main]
async fn main() {
    // Load configuration from environment (needed for Sentry init)
    let config = StorefrontConfig::from_env().expect("Failed to load configuration");

    // Initialize Sentry (must be done before tracing subscriber)
    let _sentry_guard = init_sentry(&config);

    // Initialize structured logging with service metadata
    let service_metadata = logging::ServiceMetadata::from_env("storefront");
    logging::init_tracing(&service_metadata);

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
    let state = AppState::new(
        config.clone(),
        pool,
        content_dir,
        config.claude.as_ref(),
        config.openai.as_ref(),
    )
    .expect("Failed to initialize application state");

    if state.is_chat_enabled() {
        tracing::info!("AI chat support enabled");
    } else {
        tracing::info!(
            "AI chat support disabled (missing CLAUDE_API_KEY, OPENAI_API_KEY, or TURNSTILE keys)"
        );
    }

    // Start building search index in background
    state.start_search_indexing();
    tracing::info!("Search index build started (async)");

    // Spawn dedicated health check listener (plain HTTP, no middleware)
    tokio::spawn(spawn_health_listener(state.clone(), config.health_port));

    // Create metrics layer
    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();
    tokio::spawn(serve_metrics(metric_handle));

    // Create session layer
    let session_layer = middleware::create_session_layer(state.pool(), state.config());

    // Build router with cache-controlled static file serving
    let app = Router::new()
        .merge(routes::routes())
        .merge(build_static_routes())
        .layer(session_layer)
        .layer(axum::middleware::from_fn(
            middleware::security_headers_middleware,
        ))
        .layer(axum::middleware::from_fn(middleware::csp_nonce_middleware))
        .layer(from_fn(middleware::request_id_middleware))
        .layer(prometheus_layer)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(logging::make_http_span)
                .on_response(logging::on_http_response)
                .on_failure(logging::on_http_failure),
        )
        .with_state(state)
        // Catch panics and log them before returning 500
        .layer(CatchPanicLayer::custom(handle_panic))
        // Sentry layers: NewSentryLayer must be outermost (last .layer() call)
        // so it creates a per-request hub BEFORE SentryHttpLayer sets HTTP context.
        // Reversed ordering causes scope leak between concurrent requests.
        .layer(sentry_tower::SentryHttpLayer::new().enable_transaction())
        .layer(sentry_tower::NewSentryLayer::new_from_top());

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
/// Verifies database connectivity (with a 5-second timeout) and search index
/// availability before returning OK. Returns 503 with a diagnostic body if
/// any dependency is unhealthy.
async fn readiness(State(state): State<AppState>) -> (StatusCode, &'static str) {
    match tokio::time::timeout(
        Duration::from_secs(5),
        sqlx::query("SELECT 1").fetch_one(state.pool()),
    )
    .await
    {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "Readiness check: database query failed");
            return (StatusCode::SERVICE_UNAVAILABLE, "database unavailable");
        }
        Err(_) => {
            tracing::warn!("Readiness check: database query timed out (5s)");
            return (StatusCode::SERVICE_UNAVAILABLE, "database timeout");
        }
    }

    if !state.search().is_ready() {
        tracing::warn!("Readiness check: search index not ready");
        return (StatusCode::SERVICE_UNAVAILABLE, "search index not ready");
    }

    (StatusCode::OK, "ok")
}

/// Spawn a plain HTTP health check listener for Fly.io.
///
/// Runs on a dedicated port without session, security, or tracing middleware.
/// This ensures health checks are fast, low-overhead, and never blocked by
/// middleware issues.
async fn spawn_health_listener(state: AppState, port: u16) {
    let app = Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(readiness))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "health check listener started");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind health check listener");

    axum::serve(listener, app)
        .await
        .expect("Health check listener error");
}

async fn serve_metrics(handle: axum_prometheus::metrics_exporter_prometheus::PrometheusHandle) {
    let app = Router::new().route("/metrics", get(|| async move { handle.render() }));
    let addr = SocketAddr::from(([0, 0, 0, 0], 9090));
    tracing::info!(%addr, "metrics endpoint started");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind metrics listener");
    axum::serve(listener, app)
        .await
        .expect("Metrics listener error");
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

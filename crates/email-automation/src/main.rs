//! Naked Pineapple Email Automation — background service for email workflows.
//!
//! This binary polls Microsoft 365 shared mailboxes for inbound emails,
//! classifies them with Claude AI, routes them for human review via Slack,
//! and manages automated outbound email workflows.
//!
//! # Architecture
//!
//! - Connects to `np_admin` database (shared with admin binary)
//! - Polls M365 shared mailboxes on a configurable interval
//! - Runs periodic tasks via a scheduler (abandoned carts, low stock, etc.)
//! - Exposes a health check endpoint on port 9092
//! - Accepts Slack interactive webhooks for approve/reject actions
//!
//! # Security
//!
//! - No user-facing UI — outbound-only service
//! - Single instance deployment (no duplicate processing)

#![cfg_attr(not(test), forbid(unsafe_code))]

use std::net::SocketAddr;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Router, routing::get, routing::post};
use naked_pineapple_services::claude::ClaudeClient;
use naked_pineapple_services::klaviyo::KlaviyoClient;
use naked_pineapple_services::slack::SlackClient;

mod config;
mod db;
mod error;
mod logging;
mod microsoft_graph;
mod outbound;
mod scheduler;
mod shopify;
mod slack;
mod state;
mod triage;
mod workflows;

use config::AutomationConfig;
use microsoft_graph::M365Client;
use scheduler::Scheduler;
use state::AppState;

/// Initialize Sentry error tracking and return guard that must be kept alive.
fn init_sentry(config: &AutomationConfig) -> Option<sentry::ClientInitGuard> {
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
            attach_stacktrace: true,
            ..Default::default()
        },
    ));

    tracing::info!("Sentry initialized");
    Some(guard)
}

#[tokio::main]
async fn main() {
    let config = AutomationConfig::from_env().expect("Failed to load configuration");

    let _sentry_guard = init_sentry(&config);

    let service_metadata = logging::ServiceMetadata::from_env("email-automation");
    logging::init_tracing(&service_metadata);

    tracing::info!("starting email automation service");

    let pool = db::create_pool(&config.database_url)
        .await
        .expect("Failed to create database pool");
    tracing::info!("database pool created");

    let m365 = M365Client::new(&config.m365);
    tracing::info!(
        mailboxes = ?config.m365.shared_mailboxes,
        "Microsoft Graph client initialized"
    );

    let claude = ClaudeClient::new(&config.claude);
    tracing::info!("Claude AI client initialized");

    let slack_client = config.slack.as_ref().map(|slack_config| {
        let client = SlackClient::new(
            slack_config.bot_token.clone(),
            slack_config.signing_secret.clone(),
            slack_config.channel_id.clone(),
        );
        tracing::info!("Slack client initialized");
        client
    });

    let klaviyo_client = config.klaviyo.as_ref().and_then(|klaviyo_config| {
        match KlaviyoClient::new(klaviyo_config) {
            Ok(client) => {
                tracing::info!("Klaviyo client initialized");
                Some(client)
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to initialize Klaviyo client, continuing without");
                None
            }
        }
    });

    let shopify_client = if let Some(shopify_config) = &config.shopify {
        let client = shopify::ShopifyClient::new(shopify_config);
        if let Err(e) = client.load_token(&pool).await {
            tracing::warn!(error = %e, "failed to load Shopify token, continuing without");
        } else {
            tracing::info!("Shopify client initialized");
        }
        Some(client)
    } else {
        tracing::info!("Shopify not configured, order/product lookups disabled");
        None
    };

    let health_port = config.health_port;
    let state = AppState::new(state::AppStateParams {
        config,
        pool: pool.clone(),
        m365,
        claude,
        slack: slack_client,
        klaviyo: klaviyo_client,
        shopify: shopify_client,
    });

    // Graceful shutdown channel
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Spawn HTTP listener (health checks + Slack webhook)
    tokio::spawn(spawn_http_listener(state.clone(), health_port));

    // Spawn scheduler
    let scheduler = Scheduler::new(state);
    let scheduler_handle = tokio::spawn(scheduler.run(shutdown_rx));

    // Wait for shutdown signal
    shutdown_signal().await;
    let _ = shutdown_tx.send(true);

    // Give the scheduler a moment to finish its current task
    let timeout = tokio::time::timeout(Duration::from_secs(30), scheduler_handle);
    match timeout.await {
        Ok(Ok(())) => tracing::info!("scheduler stopped cleanly"),
        Ok(Err(e)) => tracing::error!(error = %e, "scheduler task panicked"),
        Err(_) => tracing::warn!("scheduler did not stop within 30s timeout"),
    }

    tracing::info!("email automation service shut down");
}

/// Liveness health check.
async fn health() -> &'static str {
    "ok"
}

/// Readiness check — verifies database connectivity.
async fn health_readiness(State(state): State<AppState>) -> (StatusCode, &'static str) {
    match tokio::time::timeout(
        Duration::from_secs(5),
        sqlx::query("SELECT 1").fetch_one(state.pool()),
    )
    .await
    {
        Ok(Ok(_)) => (StatusCode::OK, "ok"),
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "readiness check: database query failed");
            (StatusCode::SERVICE_UNAVAILABLE, "database unavailable")
        }
        Err(_) => {
            tracing::warn!("readiness check: database query timed out (5s)");
            (StatusCode::SERVICE_UNAVAILABLE, "database timeout")
        }
    }
}

/// Spawn the HTTP listener for health checks and Slack webhooks.
async fn spawn_http_listener(state: AppState, port: u16) {
    let app = Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(health_readiness))
        .route(
            "/slack/interactions",
            post(slack::webhook::handle_interaction),
        )
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "HTTP listener started (health + webhook)");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind HTTP listener");

    axum::serve(listener, app)
        .await
        .expect("HTTP listener error");
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

    tracing::info!("shutdown signal received, starting graceful shutdown");
}

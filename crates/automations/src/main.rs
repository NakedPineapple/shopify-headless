//! Naked Pineapple Automations — background service for automated workflows.
//!
//! This binary polls Microsoft 365 shared mailboxes for inbound emails,
//! classifies them with Claude AI, routes them for human review via Slack,
//! and manages automated outbound workflows.
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
//! - Two separate HTTP listeners isolate public webhook traffic from internal endpoints
//! - Public webhook handlers use a restricted-privilege database connection
//! - Single instance deployment (no duplicate processing)

#![cfg_attr(not(test), forbid(unsafe_code))]

use std::net::SocketAddr;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Router, routing::get, routing::post};
use naked_pineapple_services::claude::ClaudeClient;
use naked_pineapple_services::email::EmailService;
use naked_pineapple_services::klaviyo::KlaviyoClient;
use naked_pineapple_services::slack::SlackClient;

mod amazon;
mod config;
mod db;
mod error;
mod logging;
mod meta;
mod outbound;
mod pinterest;
mod scheduler;
mod shopify;
mod slack;
mod state;
mod tiktok;
mod triage;
mod webhooks;
mod workflows;

use config::AutomationConfig;
use naked_pineapple_services::microsoft_graph::M365Client;
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

    let service_metadata = logging::ServiceMetadata::from_env("automations");
    logging::init_tracing(&service_metadata);

    tracing::info!("starting automations service");

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

    let email_service = config.email.as_ref().and_then(|email_config| {
        match EmailService::new(email_config) {
            Ok(service) => {
                tracing::info!("SMTP email service initialized");
                Some(service)
            }
            Err(e) => {
                tracing::warn!(error = %e, "SMTP email service not available, continuing without");
                None
            }
        }
    });

    let (amazon_client, meta_client, tiktok_client, pinterest_client) =
        load_channel_clients(&pool).await;

    let support_pool = create_support_pool(&config).await;
    let health_port = config.health_port;
    let webhook_config = config.webhook.clone();
    let state = AppState::new(state::AppStateParams {
        config,
        pool: pool.clone(),
        support_pool,
        m365,
        claude,
        slack: slack_client,
        klaviyo: klaviyo_client,
        shopify: shopify_client,
        amazon: amazon_client,
        meta: meta_client,
        tiktok: tiktok_client,
        pinterest: pinterest_client,
        email_service,
    });

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(spawn_internal_listener(state.clone(), health_port));
    start_webhook_listener(&state, webhook_config.as_ref()).await;

    let scheduler = Scheduler::new(state);
    let scheduler_handle = tokio::spawn(scheduler.run(shutdown_rx));
    await_graceful_shutdown(shutdown_tx, scheduler_handle).await;
}

/// Load all channel-specific API clients (Amazon, Meta, TikTok, Pinterest).
async fn load_channel_clients(
    pool: &sqlx::PgPool,
) -> (
    Option<naked_pineapple_services::amazon_sp::AmazonSpClient>,
    Option<naked_pineapple_services::meta_commerce::MetaCommerceClient>,
    Option<naked_pineapple_services::tiktok_shop::TikTokShopClient>,
    Option<naked_pineapple_services::pinterest::PinterestClient>,
) {
    let amazon = load_amazon_client(pool).await;
    log_client_status("Amazon SP-API", amazon.is_some());

    let meta = load_meta_client(pool).await;
    log_client_status("Meta Commerce", meta.is_some());

    let tiktok = load_tiktok_client(pool).await;
    log_client_status("TikTok Shop", tiktok.is_some());

    let pinterest = load_pinterest_client(pool).await;
    log_client_status("Pinterest", pinterest.is_some());

    (amazon, meta, tiktok, pinterest)
}

/// Log whether a channel client loaded successfully.
fn log_client_status(name: &str, loaded: bool) {
    if loaded {
        tracing::info!("{name} client initialized");
    } else {
        tracing::info!("{name} not configured, sync disabled");
    }
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

/// Start the public webhook listener and reconcile Shopify webhook subscriptions.
async fn start_webhook_listener(state: &AppState, webhook_config: Option<&config::WebhookConfig>) {
    let Some(wh_config) = webhook_config else {
        tracing::info!("WEBHOOK_DATABASE_URL not set, public webhook listener disabled");
        return;
    };

    match db::create_pool(&wh_config.database_url).await {
        Ok(webhook_pool) => {
            let webhook_state = webhooks::state::WebhookState::new(webhook_pool, wh_config);
            tokio::spawn(spawn_webhook_listener(webhook_state, wh_config.port));
            tracing::info!(port = wh_config.port, "public webhook listener started");
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "failed to create webhook database pool, public webhooks disabled"
            );
            return;
        }
    }

    if let Some(shopify) = state.shopify()
        && let Some(base_url) = &wh_config.base_url
        && let Err(e) = shopify::webhook_subscriptions::reconcile(shopify, base_url).await
    {
        tracing::warn!(error = %e, "failed to reconcile Shopify webhook subscriptions");
    }
}

/// Create the optional support pool for storefront database access.
async fn create_support_pool(config: &AutomationConfig) -> Option<sqlx::PgPool> {
    let url = config.storefront_database_url.as_ref()?;
    match db::create_pool(url).await {
        Ok(p) => {
            tracing::info!("support pool created (storefront DB)");
            Some(p)
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to create support pool, email→chat disabled");
            None
        }
    }
}

/// Wait for shutdown signal and stop the scheduler gracefully.
async fn await_graceful_shutdown(
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    scheduler_handle: tokio::task::JoinHandle<()>,
) {
    shutdown_signal().await;
    let _ = shutdown_tx.send(true);

    let timeout = tokio::time::timeout(Duration::from_secs(30), scheduler_handle);
    match timeout.await {
        Ok(Ok(())) => tracing::info!("scheduler stopped cleanly"),
        Ok(Err(e)) => tracing::error!(error = %e, "scheduler task panicked"),
        Err(_) => tracing::warn!("scheduler did not stop within 30s timeout"),
    }

    tracing::info!("automations service shut down");
}

/// Spawn the internal HTTP listener for health checks and Slack webhooks.
///
/// Binds to the health check port (default 9092). This listener is NOT exposed
/// via `http_service` in Fly.io — it is only reachable from the internal network.
async fn spawn_internal_listener(state: AppState, port: u16) {
    let app = Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(health_readiness))
        .route(
            "/slack/interactions",
            post(slack::webhook::handle_interaction),
        )
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "internal listener started (health + Slack)");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind internal listener");

    axum::serve(listener, app)
        .await
        .expect("Internal listener error");
}

/// Spawn the public webhook listener for external service webhooks.
///
/// Binds to the webhook port (default 8080). This listener is exposed via
/// `http_service` in Fly.io and uses a restricted-privilege [`WebhookState`]
/// with no access to the Shopify Admin API token or other service credentials.
async fn spawn_webhook_listener(state: webhooks::state::WebhookState, port: u16) {
    let app = webhooks::router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "public webhook listener started");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind webhook listener");

    axum::serve(listener, app)
        .await
        .expect("Webhook listener error");
}

/// Load Amazon SP-API client from stored credentials in the admin database.
///
/// Follows the same credential loading pattern as the admin crate.
async fn load_amazon_client(
    pool: &sqlx::PgPool,
) -> Option<naked_pineapple_services::amazon_sp::AmazonSpClient> {
    use naked_pineapple_services::amazon_sp::{AmazonCredentials, AmazonSpClient};

    let row = sqlx::query!(
        r"
        SELECT lwa_client_id, lwa_client_secret, lwa_refresh_token,
               aws_access_key_id, aws_secret_access_key,
               seller_id, marketplace_id
        FROM admin.amazon_sp_credentials
        WHERE account_name = 'default'
        "
    )
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some(creds)) => Some(AmazonSpClient::new(AmazonCredentials {
            lwa_client_id: creds.lwa_client_id,
            lwa_client_secret: creds.lwa_client_secret.into(),
            lwa_refresh_token: creds.lwa_refresh_token.into(),
            aws_access_key_id: creds.aws_access_key_id,
            aws_secret_access_key: creds.aws_secret_access_key.into(),
            seller_id: creds.seller_id,
            marketplace_id: creds.marketplace_id,
        })),
        Ok(None) => None,
        Err(e) => {
            tracing::error!(error = %e, "failed to load Amazon SP-API credentials");
            None
        }
    }
}

/// Load Meta Commerce client from stored credentials in the admin database.
///
/// Follows the same credential loading pattern as the Amazon SP-API client.
async fn load_meta_client(
    pool: &sqlx::PgPool,
) -> Option<naked_pineapple_services::meta_commerce::MetaCommerceClient> {
    use naked_pineapple_services::meta_commerce::{MetaCommerceClient, MetaCommerceCredentials};

    let row = sqlx::query!(
        r"
        SELECT app_id, app_secret, page_access_token, page_id,
               commerce_account_id, catalog_id
        FROM admin.meta_commerce_credentials
        WHERE account_name = 'default'
        "
    )
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some(creds)) => Some(MetaCommerceClient::new(MetaCommerceCredentials {
            app_id: creds.app_id,
            app_secret: creds.app_secret.into(),
            page_access_token: creds.page_access_token.into(),
            page_id: creds.page_id,
            commerce_account_id: creds.commerce_account_id,
            catalog_id: creds.catalog_id,
        })),
        Ok(None) => None,
        Err(e) => {
            tracing::error!(error = %e, "failed to load Meta Commerce credentials");
            None
        }
    }
}

/// Load TikTok Shop client from stored credentials in the admin database.
///
/// Follows the same credential loading pattern as the Meta Commerce client.
async fn load_tiktok_client(
    pool: &sqlx::PgPool,
) -> Option<naked_pineapple_services::tiktok_shop::TikTokShopClient> {
    use naked_pineapple_services::tiktok_shop::{TikTokShopClient, TikTokShopCredentials};

    let row = sqlx::query!(
        r"
        SELECT app_key, app_secret, access_token, refresh_token,
               shop_id, shop_cipher
        FROM admin.tiktok_shop_credentials
        WHERE account_name = 'default'
        "
    )
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some(creds)) => Some(TikTokShopClient::new(TikTokShopCredentials {
            app_key: creds.app_key,
            app_secret: creds.app_secret.into(),
            access_token: creds.access_token.into(),
            refresh_token: creds.refresh_token.into(),
            shop_id: creds.shop_id,
            shop_cipher: creds.shop_cipher,
        })),
        Ok(None) => None,
        Err(e) => {
            tracing::error!(error = %e, "failed to load TikTok Shop credentials");
            None
        }
    }
}

/// Load Pinterest client from stored credentials in the admin database.
async fn load_pinterest_client(
    pool: &sqlx::PgPool,
) -> Option<naked_pineapple_services::pinterest::PinterestClient> {
    use naked_pineapple_services::pinterest::{PinterestClient, PinterestCredentials};

    let row = sqlx::query!(
        r"
        SELECT app_id, app_secret, access_token, refresh_token, ad_account_id
        FROM admin.pinterest_credentials
        WHERE account_name = 'default'
        "
    )
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some(creds)) => Some(PinterestClient::new(PinterestCredentials {
            app_id: creds.app_id,
            app_secret: creds.app_secret.into(),
            access_token: creds.access_token.into(),
            refresh_token: creds.refresh_token.into(),
            ad_account_id: creds.ad_account_id,
        })),
        Ok(None) => None,
        Err(e) => {
            tracing::error!(error = %e, "failed to load Pinterest credentials");
            None
        }
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

    tracing::info!("shutdown signal received, starting graceful shutdown");
}

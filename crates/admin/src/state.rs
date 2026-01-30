//! Application state shared across handlers.

use std::sync::Arc;

use secrecy::ExposeSecret;
use sqlx::PgPool;
use url::Url;
use webauthn_rs::prelude::*;

use crate::config::AdminConfig;
use crate::db::ShopifyTokenRepository;
use crate::services::EmailService;
use crate::shopify::{AdminClient, OAuthToken};
use crate::slack::SlackClient;

/// Error that can occur when creating `AppState`.
#[derive(Debug, thiserror::Error)]
pub enum AppStateError {
    /// `WebAuthn` initialization failed.
    #[error("webauthn initialization failed: {0}")]
    WebAuthn(#[from] WebauthnError),

    /// Invalid URL configuration.
    #[error("invalid base URL: {0}")]
    InvalidUrl(String),

    /// Email service initialization failed.
    #[error("email service initialization failed: {0}")]
    Email(String),
}

/// Application state shared across all handlers.
///
/// This struct is cheaply cloneable via `Arc` and provides access to
/// shared resources like database connections and configuration.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    config: AdminConfig,
    pool: PgPool,
    shopify: AdminClient,
    slack: Option<SlackClient>,
    webauthn: Webauthn,
    email_service: Option<EmailService>,
}

impl AppState {
    /// Create a new application state.
    ///
    /// Loads any existing Shopify OAuth token from the database.
    ///
    /// # Arguments
    ///
    /// * `config` - Admin configuration
    /// * `pool` - `PostgreSQL` connection pool
    ///
    /// # Errors
    ///
    /// Returns `AppStateError` if `WebAuthn` initialization fails.
    pub async fn new(config: AdminConfig, pool: PgPool) -> Result<Self, AppStateError> {
        let shopify = AdminClient::new(&config.shopify);

        // Load OAuth token from database if available
        let shop = &config.shopify.store;
        let repo = ShopifyTokenRepository::new(&pool);
        match repo.get_by_shop(shop).await {
            Ok(Some(token)) => {
                tracing::info!(shop = %shop, "Loaded Shopify OAuth token from database");
                shopify
                    .set_token(OAuthToken {
                        access_token: token.access_token.expose_secret().to_string(),
                        scope: token.scopes.join(","),
                        obtained_at: token.obtained_at,
                        shop: token.shop,
                    })
                    .await;
            }
            Ok(None) => {
                tracing::warn!(
                    shop = %shop,
                    "No Shopify OAuth token found - authorization required via /settings/shopify"
                );
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to load Shopify OAuth token from database");
            }
        }

        // Initialize WebAuthn
        let base_url =
            Url::parse(&config.base_url).map_err(|e| AppStateError::InvalidUrl(e.to_string()))?;

        let rp_id = base_url
            .host_str()
            .ok_or_else(|| AppStateError::InvalidUrl("no host in base URL".to_owned()))?
            .to_owned();

        let webauthn = WebauthnBuilder::new(&rp_id, &base_url)?
            .rp_name("Naked Pineapple Admin")
            .allow_subdomains(false)
            .build()?;

        // Initialize email service (optional - dev mode works without it)
        let email_service = match EmailService::new(&config.email) {
            Ok(service) => {
                tracing::info!("Email service initialized");
                Some(service)
            }
            Err(e) => {
                tracing::warn!(error = %e, "Email service not available - running in dev mode");
                None
            }
        };

        // Initialize Slack client (optional - confirmations disabled if not configured)
        let slack = config.slack.as_ref().map(|slack_config| {
            tracing::info!("Slack integration initialized");
            SlackClient::new(
                slack_config.bot_token.clone(),
                slack_config.signing_secret.clone(),
                slack_config.channel_id.clone(),
            )
        });

        if slack.is_none() {
            tracing::warn!(
                "Slack not configured - write operations will execute without confirmation"
            );
        }

        Ok(Self {
            inner: Arc::new(AppStateInner {
                config,
                pool,
                shopify,
                slack,
                webauthn,
                email_service,
            }),
        })
    }

    /// Get a reference to the admin configuration.
    #[must_use]
    pub fn config(&self) -> &AdminConfig {
        &self.inner.config
    }

    /// Get a reference to the database connection pool.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    /// Get a reference to the Shopify Admin API client.
    #[must_use]
    pub fn shopify(&self) -> &AdminClient {
        &self.inner.shopify
    }

    /// Get a reference to the `WebAuthn` instance.
    #[must_use]
    pub fn webauthn(&self) -> &Webauthn {
        &self.inner.webauthn
    }

    /// Get a reference to the email service (if configured).
    #[must_use]
    pub fn email_service(&self) -> Option<&EmailService> {
        self.inner.email_service.as_ref()
    }

    /// Get a reference to the Slack client (if configured).
    #[must_use]
    pub fn slack(&self) -> Option<&SlackClient> {
        self.inner.slack.as_ref()
    }

    /// Get the database pool (convenience for cloning).
    #[must_use]
    pub fn db(&self) -> PgPool {
        self.inner.pool.clone()
    }
}

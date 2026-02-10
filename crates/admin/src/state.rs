//! Application state shared across handlers.

use std::sync::Arc;

use secrecy::ExposeSecret;
use sqlx::PgPool;
use url::Url;
use webauthn_rs::prelude::*;

use naked_pineapple_services::openai::EmbeddingClient;

use crate::config::AdminConfig;
use crate::db::{ShipHeroCredentialsRepository, ShopifyTokenRepository};
use crate::services::EmailService;
use crate::shiphero::ShipHeroClient;
use crate::shiphero::auth::ShipHeroToken;
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
    support_pool: Option<PgPool>,
    embedding: Option<EmbeddingClient>,
    shopify: AdminClient,
    shiphero: Option<ShipHeroClient>,
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

        // Initialize WebAuthn with multi-origin support (Related Origin Requests)
        let primary_origin_str = config.primary_origin();
        let primary_url = Url::parse(&primary_origin_str)
            .map_err(|e| AppStateError::InvalidUrl(e.to_string()))?;
        let rp_id = &config.primary_host;

        let mut builder = WebauthnBuilder::new(rp_id, &primary_url)?
            .rp_name("Naked Pineapple Admin")
            .allow_subdomains(false);

        for host in &config.hosts {
            if host != &config.primary_host {
                let origin = Url::parse(&config.origin_for(host))
                    .map_err(|e| AppStateError::InvalidUrl(e.to_string()))?;
                builder = builder.append_allowed_origin(&origin);
            }
        }

        let webauthn = builder.build()?;

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

        // Initialize ShipHero client (optional - load credentials from database if available)
        let shiphero = Self::load_shiphero_client(&pool).await;
        if shiphero.is_some() {
            tracing::info!("ShipHero client initialized from stored credentials");
        } else {
            tracing::info!(
                "ShipHero not configured - warehouse features disabled until credentials added via /settings/shiphero"
            );
        }

        let (support_pool, embedding) = Self::init_support(&config).await;

        Ok(Self {
            inner: Arc::new(AppStateInner {
                config,
                pool,
                support_pool,
                embedding,
                shopify,
                shiphero,
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

    /// Get a reference to the support pool (storefront DB, if configured).
    #[must_use]
    pub fn support_pool(&self) -> Option<&PgPool> {
        self.inner.support_pool.as_ref()
    }

    /// Whether support inbox features are available.
    #[must_use]
    pub fn is_support_enabled(&self) -> bool {
        self.inner.support_pool.is_some()
    }

    /// Get a reference to the embedding client (if configured).
    #[must_use]
    pub fn embedding(&self) -> Option<&EmbeddingClient> {
        self.inner.embedding.as_ref()
    }

    /// Get a reference to the `ShipHero` client (if configured).
    #[must_use]
    pub fn shiphero(&self) -> Option<&ShipHeroClient> {
        self.inner.shiphero.as_ref()
    }

    /// Initialize the support pool and embedding client.
    async fn init_support(config: &AdminConfig) -> (Option<PgPool>, Option<EmbeddingClient>) {
        let support_pool = if let Some(ref url) = config.storefront_database_url {
            match crate::db::create_pool(url).await {
                Ok(p) => {
                    tracing::info!("Support pool initialized (storefront DB connection)");
                    Some(p)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to connect to storefront DB — support inbox disabled");
                    None
                }
            }
        } else {
            tracing::info!("STOREFRONT_DATABASE_URL not set — support inbox disabled");
            None
        };

        let embedding = config.openai.as_ref().map(|c| EmbeddingClient::new(&c.api_key));

        (support_pool, embedding)
    }

    /// Load `ShipHero` client from stored credentials.
    async fn load_shiphero_client(pool: &PgPool) -> Option<ShipHeroClient> {
        let repo = ShipHeroCredentialsRepository::new(pool);

        match repo.get_default().await {
            Ok(Some(creds)) => {
                // Check if token is still valid
                let now = chrono::Utc::now().timestamp();
                if now >= creds.access_token_expires_at - 60 {
                    tracing::warn!(
                        "ShipHero token has expired - re-authentication required via /settings/shiphero"
                    );
                    return None;
                }

                let token = ShipHeroToken {
                    access_token: creds.access_token,
                    refresh_token: creds.refresh_token,
                    access_token_expires_at: creds.access_token_expires_at,
                    refresh_token_expires_at: creds.refresh_token_expires_at,
                };

                Some(ShipHeroClient::with_token(token))
            }
            Ok(None) => None,
            Err(e) => {
                tracing::error!(error = %e, "Failed to load ShipHero credentials from database");
                None
            }
        }
    }
}

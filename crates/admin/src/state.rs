//! Application state shared across handlers.

use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;
use secrecy::ExposeSecret;
use sqlx::PgPool;
use url::Url;
use webauthn_rs::prelude::*;

use naked_pineapple_services::amazon_sp::{AmazonSpClient, InventorySummary, PricingResult};
use naked_pineapple_services::faire::FaireClient;
use naked_pineapple_services::google_merchant::GoogleMerchantClient;
use naked_pineapple_services::judgeme::JudgemeClient;
use naked_pineapple_services::meta_commerce::MetaCommerceClient;
use naked_pineapple_services::microsoft_graph::M365Client;
use naked_pineapple_services::openai::EmbeddingClient;
use naked_pineapple_services::pinterest::PinterestClient;
use naked_pineapple_services::tiktok_shop::TikTokShopClient;

use crate::config::AdminConfig;
use crate::db::{
    AmazonSpCredentialsRepository, ShipHeroCredentialsRepository, ShopifyTokenRepository,
};
use crate::r2::R2Client;
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
    amazon: Option<AmazonSpClient>,
    meta: Option<MetaCommerceClient>,
    tiktok: Option<TikTokShopClient>,
    pinterest: Option<PinterestClient>,
    google: Option<GoogleMerchantClient>,
    faire: Option<FaireClient>,
    fba_cache: Cache<String, Vec<InventorySummary>>,
    pricing_cache: Cache<String, Vec<PricingResult>>,
    slack: Option<SlackClient>,
    m365: Option<M365Client>,
    webauthn: Webauthn,
    email_service: Option<EmailService>,
    r2: Option<R2Client>,
    r2_gallery: Option<R2Client>,
    judgeme: Option<JudgemeClient>,
}

/// Bundle of optional integrations initialized during startup.
struct Integrations {
    support_pool: Option<PgPool>,
    embedding: Option<EmbeddingClient>,
    shiphero: Option<ShipHeroClient>,
    amazon: Option<AmazonSpClient>,
    meta: Option<MetaCommerceClient>,
    tiktok: Option<TikTokShopClient>,
    pinterest: Option<PinterestClient>,
    google: Option<GoogleMerchantClient>,
    faire: Option<FaireClient>,
    slack: Option<SlackClient>,
    m365: Option<M365Client>,
    r2: Option<R2Client>,
    r2_gallery: Option<R2Client>,
    judgeme: Option<JudgemeClient>,
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
            .rp_name("Pineapple Skin Co. Admin")
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

        let integrations = Self::init_integrations(&config, &pool).await;

        let fba_cache = Cache::builder()
            .max_capacity(10)
            .time_to_live(Duration::from_mins(5))
            .build();

        let pricing_cache = Cache::builder()
            .max_capacity(100)
            .time_to_live(Duration::from_mins(5))
            .build();

        Ok(Self {
            inner: Arc::new(AppStateInner {
                config,
                pool,
                support_pool: integrations.support_pool,
                embedding: integrations.embedding,
                shopify,
                shiphero: integrations.shiphero,
                amazon: integrations.amazon,
                meta: integrations.meta,
                tiktok: integrations.tiktok,
                pinterest: integrations.pinterest,
                google: integrations.google,
                faire: integrations.faire,
                fba_cache,
                pricing_cache,
                slack: integrations.slack,
                m365: integrations.m365,
                webauthn,
                email_service,
                r2: integrations.r2,
                r2_gallery: integrations.r2_gallery,
                judgeme: integrations.judgeme,
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

    /// Get a reference to the Microsoft 365 client (if configured).
    #[must_use]
    pub fn m365(&self) -> Option<&M365Client> {
        self.inner.m365.as_ref()
    }

    /// Get a reference to the R2 client (if configured).
    #[must_use]
    pub fn r2(&self) -> Option<&R2Client> {
        self.inner.r2.as_ref()
    }

    /// Get a reference to the R2 gallery client for original images (if configured).
    #[must_use]
    pub fn r2_gallery(&self) -> Option<&R2Client> {
        self.inner.r2_gallery.as_ref()
    }

    /// Get a reference to the Judge.me client (if configured).
    #[must_use]
    pub fn judgeme(&self) -> Option<&JudgemeClient> {
        self.inner.judgeme.as_ref()
    }

    /// Get a reference to the Amazon SP-API client (if configured).
    #[must_use]
    pub fn amazon(&self) -> Option<&AmazonSpClient> {
        self.inner.amazon.as_ref()
    }

    /// Get a reference to the Meta Commerce client (if configured).
    #[must_use]
    pub fn meta(&self) -> Option<&MetaCommerceClient> {
        self.inner.meta.as_ref()
    }

    /// Get a reference to the TikTok Shop client (if configured).
    #[must_use]
    pub fn tiktok(&self) -> Option<&TikTokShopClient> {
        self.inner.tiktok.as_ref()
    }

    /// Get a reference to the Pinterest client (if configured).
    #[must_use]
    pub fn pinterest(&self) -> Option<&PinterestClient> {
        self.inner.pinterest.as_ref()
    }

    /// Get a reference to the Google Merchant Center client (if configured).
    #[must_use]
    pub fn google(&self) -> Option<&GoogleMerchantClient> {
        self.inner.google.as_ref()
    }

    /// Get a reference to the Faire client (if configured).
    #[must_use]
    pub fn faire(&self) -> Option<&FaireClient> {
        self.inner.faire.as_ref()
    }

    /// Get a reference to the FBA inventory cache (5-min TTL).
    #[must_use]
    pub fn fba_cache(&self) -> &Cache<String, Vec<InventorySummary>> {
        &self.inner.fba_cache
    }

    /// Get a reference to the competitive pricing cache (5-min TTL).
    #[must_use]
    pub fn pricing_cache(&self) -> &Cache<String, Vec<PricingResult>> {
        &self.inner.pricing_cache
    }

    /// Initialize optional integrations (Slack, `ShipHero`, M365, R2, support pool, embeddings).
    async fn init_integrations(config: &AdminConfig, pool: &PgPool) -> Integrations {
        let slack = Self::init_slack(config);
        let shiphero = Self::init_shiphero(pool).await;
        let m365 = Self::init_m365(config);
        let (support_pool, embedding) = Self::init_support(config).await;
        let (r2, r2_gallery) = Self::init_r2(config);
        let judgeme = Self::init_judgeme(config);
        let amazon = Self::init_amazon(pool).await;
        let meta = Self::init_meta(pool).await;
        let tiktok = Self::init_tiktok(pool).await;
        let pinterest = Self::init_pinterest(pool).await;
        let google = Self::init_google(pool).await;
        let faire = Self::init_faire(pool).await;

        Integrations {
            support_pool,
            embedding,
            shiphero,
            amazon,
            meta,
            tiktok,
            pinterest,
            google,
            faire,
            slack,
            m365,
            r2,
            r2_gallery,
            judgeme,
        }
    }

    /// Initialize Slack integration from config.
    fn init_slack(config: &AdminConfig) -> Option<SlackClient> {
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
        slack
    }

    /// Initialize `ShipHero` integration from stored credentials.
    async fn init_shiphero(pool: &PgPool) -> Option<ShipHeroClient> {
        let shiphero = Self::load_shiphero_client(pool).await;
        if shiphero.is_some() {
            tracing::info!("ShipHero client initialized from stored credentials");
        } else {
            tracing::info!(
                "ShipHero not configured - warehouse features disabled until credentials added via /settings/shiphero"
            );
        }
        shiphero
    }

    /// Initialize M365 integration from config.
    fn init_m365(config: &AdminConfig) -> Option<M365Client> {
        let m365 = config.m365.as_ref().map(|m365_config| {
            tracing::info!("M365 email integration initialized");
            M365Client::new(m365_config)
        });
        if m365.is_none() {
            tracing::info!("M365 not configured — email inbox sending disabled");
        }
        m365
    }

    /// Initialize R2 document and gallery storage from config.
    fn init_r2(config: &AdminConfig) -> (Option<R2Client>, Option<R2Client>) {
        let r2 = config.r2.as_ref().and_then(|r2_config| {
            r2_config.documents_bucket_name.as_ref().map(|bucket| {
                tracing::info!("R2 document storage initialized");
                R2Client::new(
                    &r2_config.account_id,
                    &r2_config.access_key_id,
                    &r2_config.secret_access_key,
                    bucket.clone(),
                )
            })
        });
        if r2.is_none() {
            tracing::info!("R2 not configured — document upload disabled");
        }

        let r2_gallery = config.r2.as_ref().and_then(|r2_config| {
            r2_config.gallery_bucket_name.as_ref().map(|bucket| {
                tracing::info!("R2 gallery storage initialized");
                R2Client::new(
                    &r2_config.account_id,
                    &r2_config.access_key_id,
                    &r2_config.secret_access_key,
                    bucket.clone(),
                )
            })
        });
        if r2_gallery.is_none() {
            tracing::info!("R2 gallery not configured — image gallery disabled");
        }

        (r2, r2_gallery)
    }

    /// Initialize Judge.me review integration from config.
    fn init_judgeme(config: &AdminConfig) -> Option<JudgemeClient> {
        let judgeme = config.judgeme.as_ref().map(|c| {
            tracing::info!("Judge.me review integration initialized");
            JudgemeClient::new(c)
        });
        if judgeme.is_none() {
            tracing::info!("Judge.me not configured — review moderation disabled");
        }
        judgeme
    }

    /// Initialize Amazon SP-API from stored credentials.
    async fn init_amazon(pool: &PgPool) -> Option<AmazonSpClient> {
        let amazon = Self::load_amazon_client(pool).await;
        if amazon.is_some() {
            tracing::info!("Amazon SP-API client initialized from stored credentials");
        } else {
            tracing::info!(
                "Amazon SP-API not configured — Amazon features disabled until credentials added via /settings/amazon"
            );
        }
        amazon
    }

    /// Initialize Meta Commerce from stored credentials.
    async fn init_meta(pool: &PgPool) -> Option<MetaCommerceClient> {
        let meta = Self::load_meta_client(pool).await;
        if meta.is_some() {
            tracing::info!("Meta Commerce client initialized from stored credentials");
        } else {
            tracing::info!(
                "Meta Commerce not configured — Meta features disabled until credentials added via /settings/meta"
            );
        }
        meta
    }

    /// Initialize Pinterest from stored credentials.
    async fn init_pinterest(pool: &PgPool) -> Option<PinterestClient> {
        let pinterest = Self::load_pinterest_client(pool).await;
        if pinterest.is_some() {
            tracing::info!("Pinterest client initialized from stored credentials");
        } else {
            tracing::info!(
                "Pinterest not configured — Pinterest features disabled until credentials added via /settings/pinterest"
            );
        }
        pinterest
    }

    /// Initialize TikTok Shop from stored credentials.
    async fn init_tiktok(pool: &PgPool) -> Option<TikTokShopClient> {
        let tiktok = Self::load_tiktok_client(pool).await;
        if tiktok.is_some() {
            tracing::info!("TikTok Shop client initialized from stored credentials");
        } else {
            tracing::info!(
                "TikTok Shop not configured — TikTok features disabled until credentials added via /settings/tiktok"
            );
        }
        tiktok
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

        let embedding = config
            .openai
            .as_ref()
            .map(|c| EmbeddingClient::new(&c.api_key));

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

    /// Load Amazon SP-API client from stored credentials.
    async fn load_amazon_client(pool: &PgPool) -> Option<AmazonSpClient> {
        use naked_pineapple_services::amazon_sp::AmazonCredentials;

        let repo = AmazonSpCredentialsRepository::new(pool);

        match repo.get_default().await {
            Ok(Some(creds)) => Some(AmazonSpClient::new(AmazonCredentials {
                lwa_client_id: creds.lwa_client_id,
                lwa_client_secret: creds.lwa_client_secret,
                lwa_refresh_token: creds.lwa_refresh_token,
                aws_access_key_id: creds.aws_access_key_id,
                aws_secret_access_key: creds.aws_secret_access_key,
                seller_id: creds.seller_id,
                marketplace_id: creds.marketplace_id,
            })),
            Ok(None) => None,
            Err(e) => {
                tracing::error!(error = %e, "Failed to load Amazon SP-API credentials from database");
                None
            }
        }
    }

    /// Load Meta Commerce client from stored credentials.
    async fn load_meta_client(pool: &PgPool) -> Option<MetaCommerceClient> {
        use crate::db::MetaCommerceCredentialsRepository;
        use naked_pineapple_services::meta_commerce::MetaCommerceCredentials;

        let repo = MetaCommerceCredentialsRepository::new(pool);

        match repo.get_default().await {
            Ok(Some(creds)) => Some(MetaCommerceClient::new(MetaCommerceCredentials {
                app_id: creds.app_id,
                app_secret: creds.app_secret,
                page_access_token: creds.page_access_token,
                page_id: creds.page_id,
                commerce_account_id: creds.commerce_account_id,
                catalog_id: creds.catalog_id,
            })),
            Ok(None) => None,
            Err(e) => {
                tracing::error!(error = %e, "Failed to load Meta Commerce credentials from database");
                None
            }
        }
    }

    /// Load TikTok Shop client from stored credentials.
    async fn load_tiktok_client(pool: &PgPool) -> Option<TikTokShopClient> {
        use crate::db::TikTokShopCredentialsRepository;
        use naked_pineapple_services::tiktok_shop::TikTokShopCredentials as ApiCredentials;

        let repo = TikTokShopCredentialsRepository::new(pool);

        match repo.get_default().await {
            Ok(Some(creds)) => Some(TikTokShopClient::new(ApiCredentials {
                app_key: creds.app_key,
                app_secret: creds.app_secret,
                access_token: creds.access_token,
                refresh_token: creds.refresh_token,
                shop_id: creds.shop_id,
                shop_cipher: creds.shop_cipher,
            })),
            Ok(None) => None,
            Err(e) => {
                tracing::error!(error = %e, "Failed to load TikTok Shop credentials from database");
                None
            }
        }
    }

    /// Initialize Google Merchant Center from stored credentials.
    async fn init_google(pool: &PgPool) -> Option<GoogleMerchantClient> {
        let google = Self::load_google_client(pool).await;
        if google.is_some() {
            tracing::info!("Google Merchant Center client initialized from stored credentials");
        } else {
            tracing::info!(
                "Google Merchant Center not configured — Google features disabled until credentials added via /settings/google"
            );
        }
        google
    }

    /// Initialize Faire from stored credentials.
    async fn init_faire(pool: &PgPool) -> Option<FaireClient> {
        let faire = Self::load_faire_client(pool).await;
        if faire.is_some() {
            tracing::info!("Faire client initialized from stored credentials");
        } else {
            tracing::info!(
                "Faire not configured — Faire features disabled until credentials added via /settings/faire"
            );
        }
        faire
    }

    /// Load Pinterest client from stored credentials.
    async fn load_pinterest_client(pool: &PgPool) -> Option<PinterestClient> {
        use crate::db::PinterestCredentialsRepository;
        use naked_pineapple_services::pinterest::PinterestCredentials as ApiCredentials;

        let repo = PinterestCredentialsRepository::new(pool);

        match repo.get_default().await {
            Ok(Some(creds)) => Some(PinterestClient::new(ApiCredentials {
                app_id: creds.app_id,
                app_secret: creds.app_secret,
                access_token: creds.access_token,
                refresh_token: creds.refresh_token,
                ad_account_id: creds.ad_account_id,
            })),
            Ok(None) => None,
            Err(e) => {
                tracing::error!(error = %e, "Failed to load Pinterest credentials from database");
                None
            }
        }
    }

    /// Load Google Merchant Center client from stored credentials.
    async fn load_google_client(pool: &PgPool) -> Option<GoogleMerchantClient> {
        use crate::db::GoogleCredentialsRepository;
        use naked_pineapple_services::google_merchant::GoogleMerchantCredentials;

        let repo = GoogleCredentialsRepository::new(pool);

        match repo.get_default().await {
            Ok(Some(creds)) => Some(GoogleMerchantClient::new(GoogleMerchantCredentials {
                merchant_id: creds.merchant_id,
                client_id: creds.client_id,
                client_secret: creds.client_secret,
                access_token: creds.access_token,
                refresh_token: creds.refresh_token,
            })),
            Ok(None) => None,
            Err(e) => {
                tracing::error!(error = %e, "Failed to load Google Merchant Center credentials from database");
                None
            }
        }
    }

    /// Load Faire client from stored credentials.
    async fn load_faire_client(pool: &PgPool) -> Option<FaireClient> {
        use crate::db::FaireCredentialsRepository;
        use naked_pineapple_services::faire::FaireCredentials as ApiCredentials;

        let repo = FaireCredentialsRepository::new(pool);

        match repo.get_default().await {
            Ok(Some(creds)) => Some(FaireClient::new(ApiCredentials {
                brand_id: creds.brand_id,
                api_token: creds.api_token,
            })),
            Ok(None) => None,
            Err(e) => {
                tracing::error!(error = %e, "Failed to load Faire credentials from database");
                None
            }
        }
    }
}

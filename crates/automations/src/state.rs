//! Application state shared across the automations service.

use std::sync::Arc;

use naked_pineapple_services::amazon_sp::AmazonSpClient;
use naked_pineapple_services::claude::ClaudeClient;
use naked_pineapple_services::email::EmailService;
use naked_pineapple_services::klaviyo::KlaviyoClient;
use naked_pineapple_services::meta_commerce::MetaCommerceClient;
use naked_pineapple_services::pinterest::PinterestClient;
use naked_pineapple_services::slack::SlackClient;
use naked_pineapple_services::tiktok_shop::TikTokShopClient;
use sqlx::PgPool;

use crate::config::AutomationConfig;
use crate::shopify::ShopifyClient;
use naked_pineapple_services::microsoft_graph::M365Client;

/// Application state shared across scheduler tasks, health check, and webhook handler.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    config: AutomationConfig,
    pool: PgPool,
    support_pool: Option<PgPool>,
    m365: M365Client,
    claude: ClaudeClient,
    slack: Option<SlackClient>,
    klaviyo: Option<KlaviyoClient>,
    shopify: Option<ShopifyClient>,
    amazon: Option<AmazonSpClient>,
    meta: Option<MetaCommerceClient>,
    tiktok: Option<TikTokShopClient>,
    pinterest: Option<PinterestClient>,
    email_service: Option<EmailService>,
}

/// Parameters for creating a new `AppState`.
pub struct AppStateParams {
    pub config: AutomationConfig,
    pub pool: PgPool,
    pub support_pool: Option<PgPool>,
    pub m365: M365Client,
    pub claude: ClaudeClient,
    pub slack: Option<SlackClient>,
    pub klaviyo: Option<KlaviyoClient>,
    pub shopify: Option<ShopifyClient>,
    pub amazon: Option<AmazonSpClient>,
    pub meta: Option<MetaCommerceClient>,
    pub tiktok: Option<TikTokShopClient>,
    pub pinterest: Option<PinterestClient>,
    pub email_service: Option<EmailService>,
}

impl AppState {
    /// Create a new application state.
    #[must_use]
    pub fn new(params: AppStateParams) -> Self {
        Self {
            inner: Arc::new(AppStateInner {
                config: params.config,
                pool: params.pool,
                support_pool: params.support_pool,
                m365: params.m365,
                claude: params.claude,
                slack: params.slack,
                klaviyo: params.klaviyo,
                shopify: params.shopify,
                amazon: params.amazon,
                meta: params.meta,
                tiktok: params.tiktok,
                pinterest: params.pinterest,
                email_service: params.email_service,
            }),
        }
    }

    /// Get a reference to the configuration.
    #[must_use]
    pub fn config(&self) -> &AutomationConfig {
        &self.inner.config
    }

    /// Get a reference to the database connection pool.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    /// Get a reference to the Microsoft Graph client.
    #[must_use]
    pub fn m365(&self) -> &M365Client {
        &self.inner.m365
    }

    /// Get a reference to the Claude AI client.
    #[must_use]
    pub fn claude(&self) -> &ClaudeClient {
        &self.inner.claude
    }

    /// Get a reference to the Slack client (if configured).
    #[must_use]
    pub fn slack(&self) -> Option<&SlackClient> {
        self.inner.slack.as_ref()
    }

    /// Get a reference to the Klaviyo client (if configured).
    #[must_use]
    pub fn klaviyo(&self) -> Option<&KlaviyoClient> {
        self.inner.klaviyo.as_ref()
    }

    /// Get a reference to the Shopify client (if configured).
    #[must_use]
    pub fn shopify(&self) -> Option<&ShopifyClient> {
        self.inner.shopify.as_ref()
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

    /// Get a reference to the SMTP email service (if configured).
    #[must_use]
    pub fn email_service(&self) -> Option<&EmailService> {
        self.inner.email_service.as_ref()
    }

    /// Get a reference to the storefront support pool (if configured).
    #[must_use]
    pub fn support_pool(&self) -> Option<&PgPool> {
        self.inner.support_pool.as_ref()
    }
}

//! Application state shared across the email automation service.

use std::sync::Arc;

use naked_pineapple_services::claude::ClaudeClient;
use naked_pineapple_services::klaviyo::KlaviyoClient;
use naked_pineapple_services::slack::SlackClient;
use sqlx::PgPool;

use crate::config::AutomationConfig;
use crate::microsoft_graph::M365Client;
use crate::shopify::ShopifyClient;

/// Application state shared across scheduler tasks, health check, and webhook handler.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    config: AutomationConfig,
    pool: PgPool,
    m365: M365Client,
    claude: ClaudeClient,
    slack: Option<SlackClient>,
    klaviyo: Option<KlaviyoClient>,
    shopify: Option<ShopifyClient>,
}

/// Parameters for creating a new `AppState`.
pub struct AppStateParams {
    pub config: AutomationConfig,
    pub pool: PgPool,
    pub m365: M365Client,
    pub claude: ClaudeClient,
    pub slack: Option<SlackClient>,
    pub klaviyo: Option<KlaviyoClient>,
    pub shopify: Option<ShopifyClient>,
}

impl AppState {
    /// Create a new application state.
    #[must_use]
    pub fn new(params: AppStateParams) -> Self {
        Self {
            inner: Arc::new(AppStateInner {
                config: params.config,
                pool: params.pool,
                m365: params.m365,
                claude: params.claude,
                slack: params.slack,
                klaviyo: params.klaviyo,
                shopify: params.shopify,
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
}

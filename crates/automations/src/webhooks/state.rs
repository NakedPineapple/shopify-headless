//! Minimal state for public webhook handlers.
//!
//! This state is intentionally isolated from the full [`AppState`](crate::state::AppState).
//! It contains only a restricted-privilege database pool and webhook verification
//! secrets — no Shopify client, no API keys, no service credentials.

use std::sync::Arc;

use secrecy::SecretString;
use sqlx::PgPool;

use crate::config::WebhookConfig;

/// State shared across public webhook handlers.
///
/// Uses a restricted-privilege database connection that can only INSERT/SELECT
/// on `admin.webhook_event`. Has no access to `admin.shopify_token` or other
/// sensitive tables.
#[derive(Clone)]
pub struct WebhookState {
    inner: Arc<WebhookStateInner>,
}

struct WebhookStateInner {
    pool: PgPool,
    shopify_secret: Option<SecretString>,
    github_secret: Option<SecretString>,
    sentry_secret: Option<SecretString>,
    fly_token: Option<SecretString>,
    betterstack_secret: Option<SecretString>,
}

impl WebhookState {
    /// Create webhook state from config and a restricted-privilege pool.
    #[must_use]
    pub fn new(pool: PgPool, config: &WebhookConfig) -> Self {
        Self {
            inner: Arc::new(WebhookStateInner {
                pool,
                shopify_secret: config.shopify_secret.clone(),
                github_secret: config.github_secret.clone(),
                sentry_secret: config.sentry_secret.clone(),
                fly_token: config.fly_token.clone(),
                betterstack_secret: config.betterstack_secret.clone(),
            }),
        }
    }

    /// Restricted-privilege database pool.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    /// Shopify webhook HMAC secret.
    #[must_use]
    pub fn shopify_secret(&self) -> Option<&SecretString> {
        self.inner.shopify_secret.as_ref()
    }

    /// GitHub webhook HMAC secret.
    #[must_use]
    pub fn github_secret(&self) -> Option<&SecretString> {
        self.inner.github_secret.as_ref()
    }

    /// Sentry webhook secret.
    #[must_use]
    pub fn sentry_secret(&self) -> Option<&SecretString> {
        self.inner.sentry_secret.as_ref()
    }

    /// Fly.io webhook bearer token.
    #[must_use]
    pub fn fly_token(&self) -> Option<&SecretString> {
        self.inner.fly_token.as_ref()
    }

    /// Better Stack webhook secret.
    #[must_use]
    pub fn betterstack_secret(&self) -> Option<&SecretString> {
        self.inner.betterstack_secret.as_ref()
    }
}

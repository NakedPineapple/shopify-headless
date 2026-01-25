//! Application state shared across handlers.

use std::sync::Arc;

use sqlx::PgPool;
use url::Url;
use webauthn_rs::prelude::*;

use crate::config::AdminConfig;
use crate::shopify::AdminClient;

/// Error that can occur when creating `AppState`.
#[derive(Debug, thiserror::Error)]
pub enum AppStateError {
    /// `WebAuthn` initialization failed.
    #[error("webauthn initialization failed: {0}")]
    WebAuthn(#[from] WebauthnError),

    /// Invalid URL configuration.
    #[error("invalid base URL: {0}")]
    InvalidUrl(String),
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
    webauthn: Webauthn,
}

impl AppState {
    /// Create a new application state.
    ///
    /// # Arguments
    ///
    /// * `config` - Admin configuration
    /// * `pool` - `PostgreSQL` connection pool
    ///
    /// # Errors
    ///
    /// Returns `AppStateError` if `WebAuthn` initialization fails.
    pub fn new(config: AdminConfig, pool: PgPool) -> Result<Self, AppStateError> {
        let shopify = AdminClient::new(&config.shopify);

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

        Ok(Self {
            inner: Arc::new(AppStateInner {
                config,
                pool,
                shopify,
                webauthn,
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
}

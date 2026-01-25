//! Application state shared across handlers.

use std::sync::Arc;

use sqlx::PgPool;
use url::Url;
use webauthn_rs::prelude::*;

use crate::config::StorefrontConfig;
use crate::shopify::StorefrontClient;

/// Error creating `WebAuthn` configuration.
#[derive(Debug, thiserror::Error)]
pub enum WebauthnConfigError {
    #[error("invalid base_url: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("base_url must have a host")]
    MissingHost,
    #[error("webauthn error: {0}")]
    WebAuthn(#[from] WebauthnError),
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
    config: StorefrontConfig,
    pool: PgPool,
    storefront: StorefrontClient,
    webauthn: Webauthn,
}

impl AppState {
    /// Create a new application state.
    ///
    /// # Arguments
    ///
    /// * `config` - Storefront configuration
    /// * `pool` - `PostgreSQL` connection pool
    ///
    /// # Errors
    ///
    /// Returns an error if the `WebAuthn` configuration is invalid.
    pub fn new(config: StorefrontConfig, pool: PgPool) -> Result<Self, WebauthnConfigError> {
        let storefront = StorefrontClient::new(&config.shopify);
        let webauthn = create_webauthn(&config)?;

        Ok(Self {
            inner: Arc::new(AppStateInner {
                config,
                pool,
                storefront,
                webauthn,
            }),
        })
    }

    /// Get a reference to the storefront configuration.
    #[must_use]
    pub fn config(&self) -> &StorefrontConfig {
        &self.inner.config
    }

    /// Get a reference to the database connection pool.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    /// Get a reference to the Shopify Storefront API client.
    #[must_use]
    pub fn storefront(&self) -> &StorefrontClient {
        &self.inner.storefront
    }

    /// Get a reference to the `WebAuthn` configuration.
    #[must_use]
    pub fn webauthn(&self) -> &Webauthn {
        &self.inner.webauthn
    }
}

/// Create a `WebAuthn` instance from configuration.
fn create_webauthn(config: &StorefrontConfig) -> Result<Webauthn, WebauthnConfigError> {
    // Parse the base URL to get the origin and RP ID
    let url = Url::parse(&config.base_url)?;

    let rp_id = url
        .host_str()
        .ok_or(WebauthnConfigError::MissingHost)?
        .to_owned();

    let builder = WebauthnBuilder::new(&rp_id, &url)?
        .rp_name("Naked Pineapple")
        .allow_subdomains(false);

    Ok(builder.build()?)
}

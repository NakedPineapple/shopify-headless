//! Application state shared across handlers.

use std::sync::Arc;

use sqlx::PgPool;

use crate::config::StorefrontConfig;
use crate::shopify::StorefrontClient;

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
}

impl AppState {
    /// Create a new application state.
    ///
    /// # Arguments
    ///
    /// * `config` - Storefront configuration
    /// * `pool` - `PostgreSQL` connection pool
    #[must_use]
    pub fn new(config: StorefrontConfig, pool: PgPool) -> Self {
        let storefront = StorefrontClient::new(&config.shopify);
        Self {
            inner: Arc::new(AppStateInner {
                config,
                pool,
                storefront,
            }),
        }
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
}

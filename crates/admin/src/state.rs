//! Application state shared across handlers.

use std::sync::Arc;

use sqlx::PgPool;

use crate::config::AdminConfig;
use crate::shopify::AdminClient;

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
}

impl AppState {
    /// Create a new application state.
    ///
    /// # Arguments
    ///
    /// * `config` - Admin configuration
    /// * `pool` - `PostgreSQL` connection pool
    #[must_use]
    pub fn new(config: AdminConfig, pool: PgPool) -> Self {
        let shopify = AdminClient::new(&config.shopify);
        Self {
            inner: Arc::new(AppStateInner {
                config,
                pool,
                shopify,
            }),
        }
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
}

//! Application state shared across the email automation service.

use std::sync::Arc;

use sqlx::PgPool;

use crate::config::AutomationConfig;
use crate::microsoft_graph::M365Client;

/// Application state shared across scheduler tasks and health check.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    config: AutomationConfig,
    pool: PgPool,
    m365: M365Client,
}

impl AppState {
    /// Create a new application state.
    #[must_use]
    pub fn new(config: AutomationConfig, pool: PgPool, m365: M365Client) -> Self {
        Self {
            inner: Arc::new(AppStateInner { config, pool, m365 }),
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
}

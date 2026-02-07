//! Application state shared across handlers.

use std::path::Path;
use std::sync::Arc;

use sqlx::PgPool;

use crate::config::StorefrontConfig;
use crate::content::{ContentError, ContentStore};
use crate::search::SearchIndex;
use crate::shopify::{CustomerClient, StorefrontClient};

/// Error creating application state.
#[derive(Debug, thiserror::Error)]
pub enum AppStateError {
    #[error("content error: {0}")]
    Content(#[from] ContentError),
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
    customer: CustomerClient,
    content: ContentStore,
    search: SearchIndex,
}

impl AppState {
    /// Create a new application state.
    ///
    /// # Arguments
    ///
    /// * `config` - Storefront configuration
    /// * `pool` - `PostgreSQL` connection pool
    /// * `content_dir` - Path to content directory for markdown files
    ///
    /// # Errors
    ///
    /// Returns an error if content fails to load.
    pub fn new(
        config: StorefrontConfig,
        pool: PgPool,
        content_dir: &Path,
    ) -> Result<Self, AppStateError> {
        let storefront = StorefrontClient::new(&config.shopify);
        let customer = CustomerClient::new(&config.shopify);
        let content = ContentStore::load(content_dir)?;
        let search = SearchIndex::new();

        Ok(Self {
            inner: Arc::new(AppStateInner {
                config,
                pool,
                storefront,
                customer,
                content,
                search,
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

    /// Get a reference to the Shopify Customer Account API client.
    #[must_use]
    pub fn customer(&self) -> &CustomerClient {
        &self.inner.customer
    }

    /// Get a reference to the content store.
    #[must_use]
    pub fn content(&self) -> &ContentStore {
        &self.inner.content
    }

    /// Get a reference to the search index.
    #[must_use]
    pub fn search(&self) -> &SearchIndex {
        &self.inner.search
    }

    /// Start building the search index asynchronously.
    ///
    /// This spawns a background task that fetches products/collections from Shopify
    /// and indexes them along with local content. Until complete, search returns
    /// empty results.
    pub fn start_search_indexing(&self) {
        crate::search::build_index_async(
            self.inner.search.clone(),
            self.inner.storefront.clone(),
            self.inner.content.clone(),
        );
    }
}

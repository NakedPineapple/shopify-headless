//! Application state shared across handlers.

use std::path::Path;
use std::sync::Arc;

use naked_pineapple_services::claude::ClaudeClient;
use naked_pineapple_services::config::{ClaudeConfig, OpenAIConfig};
use naked_pineapple_services::openai::EmbeddingClient;
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
    claude: Option<ClaudeClient>,
    embedding: Option<EmbeddingClient>,
}

impl AppState {
    /// Create a new application state.
    ///
    /// # Arguments
    ///
    /// * `config` - Storefront configuration
    /// * `pool` - `PostgreSQL` connection pool
    /// * `content_dir` - Path to content directory for markdown files
    /// * `claude_config` - Optional Claude API configuration
    /// * `openai_config` - Optional `OpenAI` API configuration
    ///
    /// # Errors
    ///
    /// Returns an error if content fails to load.
    pub fn new(
        config: StorefrontConfig,
        pool: PgPool,
        content_dir: &Path,
        claude_config: Option<&ClaudeConfig>,
        openai_config: Option<&OpenAIConfig>,
    ) -> Result<Self, AppStateError> {
        let storefront = StorefrontClient::new(&config.shopify);
        let customer = CustomerClient::new(&config.shopify);
        let content = ContentStore::load(content_dir)?;
        let search = SearchIndex::new();
        let claude = claude_config.map(ClaudeClient::new);
        let embedding = openai_config.map(|c| EmbeddingClient::new(&c.api_key));

        Ok(Self {
            inner: Arc::new(AppStateInner {
                config,
                pool,
                storefront,
                customer,
                content,
                search,
                claude,
                embedding,
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

    /// Get a reference to the Claude API client, if configured.
    #[must_use]
    pub fn claude(&self) -> Option<&ClaudeClient> {
        self.inner.claude.as_ref()
    }

    /// Get a reference to the embedding client, if configured.
    #[must_use]
    pub fn embedding(&self) -> Option<&EmbeddingClient> {
        self.inner.embedding.as_ref()
    }

    /// Returns true if AI chat support is fully configured.
    #[must_use]
    pub fn is_chat_enabled(&self) -> bool {
        self.inner.claude.is_some()
            && self.inner.embedding.is_some()
            && self.inner.config.is_chat_enabled()
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

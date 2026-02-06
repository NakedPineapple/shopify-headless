//! Application state shared across handlers.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use sqlx::PgPool;
use url::Url;
use webauthn_rs::prelude::*;

use crate::config::StorefrontConfig;
use crate::content::{ContentError, ContentStore};
use crate::search::SearchIndex;
use crate::shopify::{CustomerClient, StorefrontClient};

/// Error creating application state.
#[derive(Debug, thiserror::Error)]
pub enum AppStateError {
    #[error("invalid base URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("base URL must have a host: {0}")]
    MissingHost(String),
    #[error("webauthn error: {0}")]
    WebAuthn(#[from] WebauthnError),
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
    webauthn_map: HashMap<String, Webauthn>,
    default_webauthn: Webauthn,
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
    /// Returns an error if the `WebAuthn` configuration is invalid or content fails to load.
    pub fn new(
        config: StorefrontConfig,
        pool: PgPool,
        content_dir: &Path,
    ) -> Result<Self, AppStateError> {
        let storefront = StorefrontClient::new(&config.shopify);
        let customer = CustomerClient::new(&config.shopify);
        let (webauthn_map, default_webauthn) = create_webauthn_map(&config)?;
        let content = ContentStore::load(content_dir)?;
        let search = SearchIndex::new();

        Ok(Self {
            inner: Arc::new(AppStateInner {
                config,
                pool,
                storefront,
                customer,
                webauthn_map,
                default_webauthn,
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

    /// Get the `WebAuthn` instance for a given host.
    ///
    /// Falls back to the default instance if the host is not found.
    #[must_use]
    pub fn webauthn_for_host(&self, host: &str) -> &Webauthn {
        self.inner
            .webauthn_map
            .get(host)
            .unwrap_or(&self.inner.default_webauthn)
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

/// Build a `WebAuthn` instance per configured domain.
///
/// Returns a map of host → `Webauthn` and the default instance (first entry).
fn create_webauthn_map(
    config: &StorefrontConfig,
) -> Result<(HashMap<String, Webauthn>, Webauthn), AppStateError> {
    let mut map = HashMap::new();
    let mut default = None;

    for (host, base_url) in &config.base_urls {
        let url = Url::parse(base_url)?;
        let rp_id = url
            .host_str()
            .ok_or_else(|| AppStateError::MissingHost(base_url.clone()))?
            .to_owned();

        let webauthn = WebauthnBuilder::new(&rp_id, &url)?
            .rp_name("Naked Pineapple")
            .allow_subdomains(false)
            .build()?;

        if default.is_none() {
            default = Some(webauthn.clone());
        }
        map.insert(host.clone(), webauthn);
    }

    // default_base_url is guaranteed to have at least one entry (validated in config),
    // so we can safely build from it as a fallback.
    let default = default.unwrap_or_else(|| {
        let url = Url::parse(&config.default_base_url).expect("default_base_url already validated");
        let rp_id = url.host_str().expect("default_base_url has host");
        WebauthnBuilder::new(rp_id, &url)
            .expect("valid webauthn config")
            .rp_name("Naked Pineapple")
            .allow_subdomains(false)
            .build()
            .expect("valid webauthn build")
    });

    Ok((map, default))
}

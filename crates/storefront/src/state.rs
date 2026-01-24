//! Application state shared across handlers.
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use sqlx::PgPool;
//! use std::sync::Arc;
//!
//! use crate::{
//!     config::StorefrontConfig,
//!     shopify::{CustomerClient, StorefrontClient},
//! };
//!
//! /// Application state shared across all handlers.
//! #[derive(Clone)]
//! pub struct AppState {
//!     inner: Arc<AppStateInner>,
//! }
//!
//! struct AppStateInner {
//!     config: StorefrontConfig,
//!     pool: PgPool,
//!     storefront_client: StorefrontClient,
//!     customer_client: CustomerClient,
//! }
//!
//! impl AppState {
//!     pub fn new(
//!         config: StorefrontConfig,
//!         pool: PgPool,
//!         storefront_client: StorefrontClient,
//!         customer_client: CustomerClient,
//!     ) -> Self {
//!         Self {
//!             inner: Arc::new(AppStateInner {
//!                 config,
//!                 pool,
//!                 storefront_client,
//!                 customer_client,
//!             }),
//!         }
//!     }
//!
//!     pub fn config(&self) -> &StorefrontConfig {
//!         &self.inner.config
//!     }
//!
//!     pub fn pool(&self) -> &PgPool {
//!         &self.inner.pool
//!     }
//!
//!     pub fn storefront(&self) -> &StorefrontClient {
//!         &self.inner.storefront_client
//!     }
//!
//!     pub fn customer(&self) -> &CustomerClient {
//!         &self.inner.customer_client
//!     }
//! }
//! ```

// TODO: Implement AppState

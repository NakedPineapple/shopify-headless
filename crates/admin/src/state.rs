//! Application state shared across handlers.
//!
//! # Future Implementation
//!
//! ```rust,ignore
//! use sqlx::PgPool;
//! use std::sync::Arc;
//!
//! use crate::{
//!     claude::ClaudeClient,
//!     config::AdminConfig,
//!     shopify::AdminClient,
//! };
//!
//! /// Application state shared across all handlers.
//! #[derive(Clone)]
//! pub struct AppState {
//!     inner: Arc<AppStateInner>,
//! }
//!
//! struct AppStateInner {
//!     config: AdminConfig,
//!     pool: PgPool,
//!     shopify_client: AdminClient,
//!     claude_client: ClaudeClient,
//! }
//!
//! impl AppState {
//!     pub fn new(
//!         config: AdminConfig,
//!         pool: PgPool,
//!         shopify_client: AdminClient,
//!         claude_client: ClaudeClient,
//!     ) -> Self {
//!         Self {
//!             inner: Arc::new(AppStateInner {
//!                 config,
//!                 pool,
//!                 shopify_client,
//!                 claude_client,
//!             }),
//!         }
//!     }
//!
//!     pub fn config(&self) -> &AdminConfig {
//!         &self.inner.config
//!     }
//!
//!     pub fn pool(&self) -> &PgPool {
//!         &self.inner.pool
//!     }
//!
//!     pub fn shopify(&self) -> &AdminClient {
//!         &self.inner.shopify_client
//!     }
//!
//!     pub fn claude(&self) -> &ClaudeClient {
//!         &self.inner.claude_client
//!     }
//! }
//! ```

// TODO: Implement AppState

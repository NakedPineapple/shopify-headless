//! Session middleware configuration.
//!
//! Sets up `PostgreSQL`-backed sessions using tower-sessions.

use secrecy::ExposeSecret;
use sqlx::PgPool;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;

use crate::config::StorefrontConfig;

/// Session cookie name.
pub const SESSION_COOKIE_NAME: &str = "np_session";

/// Session expiry time in seconds (7 days).
const SESSION_EXPIRY_SECONDS: i64 = 7 * 24 * 60 * 60;

/// Create the session layer with `PostgreSQL` store.
///
/// # Arguments
///
/// * `pool` - `PostgreSQL` connection pool
/// * `config` - Storefront configuration (for session secret)
#[must_use]
pub fn create_session_layer(
    pool: &PgPool,
    config: &StorefrontConfig,
) -> SessionManagerLayer<PostgresStore> {
    // Create the PostgreSQL session store
    // Note: The sessions table must be created via migration
    let store = PostgresStore::new(pool.clone());

    // Determine if we're in production (HTTPS)
    let is_secure = config.base_url.starts_with("https://");

    SessionManagerLayer::new(store)
        .with_name(SESSION_COOKIE_NAME)
        .with_expiry(Expiry::OnInactivity(
            tower_sessions::cookie::time::Duration::seconds(SESSION_EXPIRY_SECONDS),
        ))
        .with_secure(is_secure)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_http_only(true)
        .with_path("/")
}

//! Session middleware configuration for admin.
//!
//! Sets up `PostgreSQL`-backed sessions using tower-sessions with
//! stricter security settings (SameSite=Strict, 24hr expiry).

use sqlx::PgPool;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;

use crate::config::AdminConfig;

/// Session cookie name for admin.
pub const SESSION_COOKIE_NAME: &str = "np_admin_session";

/// Session expiry time in seconds (24 hours - stricter than storefront).
const SESSION_EXPIRY_SECONDS: i64 = 24 * 60 * 60;

/// Create the session layer with `PostgreSQL` store.
///
/// # Arguments
///
/// * `pool` - `PostgreSQL` connection pool
/// * `config` - Admin configuration (for determining HTTPS mode)
///
/// # Panics
///
/// Panics if the schema name or table name is invalid (should never happen
/// with hardcoded "admin" and "sessions" values).
#[must_use]
pub fn create_session_layer(
    pool: &PgPool,
    config: &AdminConfig,
) -> SessionManagerLayer<PostgresStore> {
    // Create the PostgreSQL session store
    // Note: The session table must be created via migration in the admin schema.
    let store = PostgresStore::new(pool.clone())
        .with_schema_name("admin")
        .expect("valid schema name")
        .with_table_name("session")
        .expect("valid table name");

    // Determine if we're in production (HTTPS)
    let is_secure = config.base_url.starts_with("https://");

    SessionManagerLayer::new(store)
        .with_name(SESSION_COOKIE_NAME)
        .with_expiry(Expiry::OnInactivity(
            tower_sessions::cookie::time::Duration::seconds(SESSION_EXPIRY_SECONDS),
        ))
        .with_secure(is_secure)
        // SameSite=Strict for admin (stricter than storefront's Lax)
        .with_same_site(tower_sessions::cookie::SameSite::Strict)
        .with_http_only(true)
        .with_path("/")
}

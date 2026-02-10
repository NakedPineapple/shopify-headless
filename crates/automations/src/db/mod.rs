//! Database operations for the automations service.
//!
//! Connects to the `np_admin` database (shared with the admin binary).
//! New tables for email triage, automation logs, and outbound queues
//! are added via admin's migration directory.

pub mod abandoned_cart;
pub mod automation_log;
pub mod email_sync_state;
pub mod inbound_email;
pub mod outbound_queue;

use std::time::Duration;

use chrono::{DateTime, Datelike, Timelike, Utc};
use secrecy::ExposeSecret;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use thiserror::Error;

/// Convert chrono `DateTime<Utc>` to `time::OffsetDateTime` for `SQLx` bind params.
///
/// When both `chrono` and `time` features are active for sqlx (the latter via
/// workspace feature unification from `tower-sessions-sqlx-store`), the offline
/// macro expansion resolves `TIMESTAMPTZ` to `time::OffsetDateTime`. This mirrors
/// the `to_time_date` pattern in `crates/admin/src/db/manufacturing.rs` for `DATE`.
pub fn to_time_offset(dt: DateTime<Utc>) -> time::OffsetDateTime {
    let date = time::Date::from_calendar_date(
        dt.year(),
        time::Month::try_from(u8::try_from(dt.month()).expect("month in range"))
            .expect("valid month"),
        u8::try_from(dt.day()).expect("day in range"),
    )
    .expect("valid date");
    let t = time::Time::from_hms_nano(
        u8::try_from(dt.hour()).expect("hour in range"),
        u8::try_from(dt.minute()).expect("minute in range"),
        u8::try_from(dt.second()).expect("second in range"),
        dt.timestamp_subsec_nanos(),
    )
    .expect("valid time");
    time::OffsetDateTime::new_utc(date, t)
}

/// Errors that can occur during repository operations.
#[derive(Debug, Error)]
pub enum RepositoryError {
    /// Database error from sqlx.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Requested entity was not found.
    #[error("not found")]
    NotFound,
}

/// Create a `PostgreSQL` connection pool.
///
/// # Errors
///
/// Returns `sqlx::Error` if the connection cannot be established.
pub async fn create_pool(database_url: &secrecy::SecretString) -> Result<PgPool, sqlx::Error> {
    let max_conn: u32 = std::env::var("DATABASE_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let min_conn: u32 = std::env::var("DATABASE_MIN_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    PgPoolOptions::new()
        .max_connections(max_conn)
        .min_connections(min_conn)
        .acquire_timeout(Duration::from_secs(10))
        .connect(database_url.expose_secret())
        .await
}

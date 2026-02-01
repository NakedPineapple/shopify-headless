//! Settings database operations.
//!
//! Handles both global and user-specific settings storage.

use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::{debug, info, instrument};

/// Error type for settings operations.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Get a global setting value.
///
/// # Errors
///
/// Returns an error if the database query fails.
#[instrument(skip(pool), fields(key = %key), level = "debug")]
pub async fn get_setting(pool: &PgPool, key: &str) -> Result<Option<JsonValue>, SettingsError> {
    debug!("Fetching global setting");
    let result = sqlx::query_scalar!(
        r#"
        SELECT value FROM admin.settings
        WHERE key = $1 AND admin_user_id IS NULL
        "#,
        key
    )
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

/// Set a global setting value.
///
/// # Errors
///
/// Returns an error if the database query fails.
#[instrument(skip(pool, value), fields(key = %key), level = "debug")]
pub async fn set_setting(pool: &PgPool, key: &str, value: &JsonValue) -> Result<(), SettingsError> {
    debug!("Setting global setting");
    sqlx::query!(
        r#"
        INSERT INTO admin.settings (key, value, admin_user_id)
        VALUES ($1, $2, NULL)
        ON CONFLICT (key, admin_user_id) DO UPDATE SET value = $2, updated_at = NOW()
        "#,
        key,
        value
    )
    .execute(pool)
    .await?;

    info!("Global setting saved");
    Ok(())
}

/// Get a user-specific setting value.
///
/// # Errors
///
/// Returns an error if the database query fails.
#[instrument(skip(pool), fields(user_id = %user_id, key = %key), level = "debug")]
pub async fn get_user_setting(
    pool: &PgPool,
    user_id: i32,
    key: &str,
) -> Result<Option<JsonValue>, SettingsError> {
    debug!("Fetching user setting");
    let result = sqlx::query_scalar!(
        r#"
        SELECT value FROM admin.settings
        WHERE key = $1 AND admin_user_id = $2
        "#,
        key,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

/// Set a user-specific setting value.
///
/// # Errors
///
/// Returns an error if the database query fails.
#[instrument(skip(pool, value), fields(user_id = %user_id, key = %key), level = "debug")]
pub async fn set_user_setting(
    pool: &PgPool,
    user_id: i32,
    key: &str,
    value: &JsonValue,
) -> Result<(), SettingsError> {
    debug!("Setting user setting");
    sqlx::query!(
        r#"
        INSERT INTO admin.settings (key, value, admin_user_id)
        VALUES ($1, $2, $3)
        ON CONFLICT (key, admin_user_id) DO UPDATE SET value = $2, updated_at = NOW()
        "#,
        key,
        value,
        user_id
    )
    .execute(pool)
    .await?;

    info!("User setting saved");
    Ok(())
}

/// Delete a user-specific setting.
///
/// # Errors
///
/// Returns an error if the database query fails.
#[instrument(skip(pool), fields(user_id = %user_id, key = %key), level = "debug")]
pub async fn delete_user_setting(
    pool: &PgPool,
    user_id: i32,
    key: &str,
) -> Result<(), SettingsError> {
    debug!("Deleting user setting");
    sqlx::query!(
        r#"
        DELETE FROM admin.settings
        WHERE key = $1 AND admin_user_id = $2
        "#,
        key,
        user_id
    )
    .execute(pool)
    .await?;

    info!("User setting deleted");
    Ok(())
}

/// Get all user-specific settings with a given prefix.
///
/// # Errors
///
/// Returns an error if the database query fails.
#[instrument(skip(pool), fields(user_id = %user_id, prefix = %prefix), level = "debug")]
pub async fn get_user_settings_by_prefix(
    pool: &PgPool,
    user_id: i32,
    prefix: &str,
) -> Result<Vec<(String, JsonValue)>, SettingsError> {
    debug!("Fetching user settings by prefix");
    let pattern = format!("{prefix}%");
    let results = sqlx::query!(
        r#"
        SELECT key, value FROM admin.settings
        WHERE admin_user_id = $1 AND key LIKE $2
        "#,
        user_id,
        pattern
    )
    .fetch_all(pool)
    .await?;

    Ok(results.into_iter().map(|r| (r.key, r.value)).collect())
}

//! Database operations for gallery image metadata.

use std::collections::HashMap;

use sqlx::PgPool;
use tracing::{debug, instrument};

use super::RepositoryError;

/// Gallery metadata row from the database.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GalleryMetadataRow {
    pub r2_key: String,
    pub alt_text: Option<String>,
    pub description: Option<String>,
    pub custom_metadata: serde_json::Value,
    pub updated_by: Option<i32>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Parameters for upserting gallery metadata.
pub struct UpsertMetadataParams {
    pub r2_key: String,
    pub alt_text: Option<String>,
    pub description: Option<String>,
    pub custom_metadata: serde_json::Value,
    pub updated_by: i32,
}

/// Get metadata for a single image by R2 key.
///
/// # Errors
///
/// Returns error if the database query fails.
#[instrument(skip(pool))]
pub async fn get_metadata(
    pool: &PgPool,
    r2_key: &str,
) -> Result<Option<GalleryMetadataRow>, RepositoryError> {
    let row = sqlx::query_as!(
        GalleryMetadataRow,
        r#"
        SELECT r2_key, alt_text, description, custom_metadata,
               updated_by,
               updated_at AS "updated_at: chrono::DateTime<chrono::Utc>"
        FROM admin.gallery_metadata
        WHERE r2_key = $1
        "#,
        r2_key
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Get metadata for multiple images by R2 keys.
///
/// Returns a map from R2 key to metadata row.
///
/// # Errors
///
/// Returns error if the database query fails.
#[instrument(skip(pool, r2_keys), fields(count = r2_keys.len()))]
pub async fn get_metadata_batch(
    pool: &PgPool,
    r2_keys: &[String],
) -> Result<HashMap<String, GalleryMetadataRow>, RepositoryError> {
    if r2_keys.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as!(
        GalleryMetadataRow,
        r#"
        SELECT r2_key, alt_text, description, custom_metadata,
               updated_by,
               updated_at AS "updated_at: chrono::DateTime<chrono::Utc>"
        FROM admin.gallery_metadata
        WHERE r2_key = ANY($1)
        "#,
        r2_keys
    )
    .fetch_all(pool)
    .await?;

    let map: HashMap<String, GalleryMetadataRow> =
        rows.into_iter().map(|r| (r.r2_key.clone(), r)).collect();

    debug!(found = map.len(), "Fetched gallery metadata batch");
    Ok(map)
}

/// Insert or update gallery metadata for an image.
///
/// # Errors
///
/// Returns error if the database query fails.
#[instrument(skip(pool, params), fields(r2_key = %params.r2_key))]
pub async fn upsert_metadata(
    pool: &PgPool,
    params: &UpsertMetadataParams,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        INSERT INTO admin.gallery_metadata (r2_key, alt_text, description, custom_metadata, updated_by)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (r2_key) DO UPDATE SET
            alt_text = EXCLUDED.alt_text,
            description = EXCLUDED.description,
            custom_metadata = EXCLUDED.custom_metadata,
            updated_by = EXCLUDED.updated_by
        "#,
        params.r2_key,
        params.alt_text,
        params.description,
        params.custom_metadata,
        params.updated_by
    )
    .execute(pool)
    .await?;

    debug!("Upserted gallery metadata");
    Ok(())
}

/// Delete metadata for a single image.
///
/// # Errors
///
/// Returns error if the database query fails.
#[instrument(skip(pool))]
pub async fn delete_metadata(pool: &PgPool, r2_key: &str) -> Result<(), RepositoryError> {
    sqlx::query!(
        "DELETE FROM admin.gallery_metadata WHERE r2_key = $1",
        r2_key
    )
    .execute(pool)
    .await?;

    debug!("Deleted gallery metadata");
    Ok(())
}

/// Delete metadata for multiple images.
///
/// # Errors
///
/// Returns error if the database query fails.
#[instrument(skip(pool, r2_keys), fields(count = r2_keys.len()))]
pub async fn delete_metadata_batch(
    pool: &PgPool,
    r2_keys: &[String],
) -> Result<(), RepositoryError> {
    if r2_keys.is_empty() {
        return Ok(());
    }

    sqlx::query!(
        "DELETE FROM admin.gallery_metadata WHERE r2_key = ANY($1)",
        r2_keys
    )
    .execute(pool)
    .await?;

    debug!("Bulk deleted gallery metadata");
    Ok(())
}

/// Update the R2 key for a metadata row (used during move/rename).
///
/// # Errors
///
/// Returns error if the database query fails.
#[instrument(skip(pool))]
pub async fn update_metadata_key(
    pool: &PgPool,
    old_key: &str,
    new_key: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.gallery_metadata
        SET r2_key = $2
        WHERE r2_key = $1
        "#,
        old_key,
        new_key
    )
    .execute(pool)
    .await?;

    debug!("Updated gallery metadata key");
    Ok(())
}

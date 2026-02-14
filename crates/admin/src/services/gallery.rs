//! Gallery service: image upload, thumbnail generation, move/rename, and deletion.

use bytes::Bytes;
use image::ImageFormat;
use sqlx::PgPool;
use thiserror::Error;
use tracing::{debug, info, instrument, warn};

use crate::db::gallery as gallery_db;
use crate::r2::{R2Client, R2Error};

/// Maximum upload size (50 MB — originals can be large).
pub const MAX_IMAGE_SIZE: usize = 50 * 1024 * 1024;

/// Maximum thumbnail dimension (width or height).
const THUMB_MAX_DIMENSION: u32 = 400;

/// R2 prefix for thumbnails.
const THUMB_PREFIX: &str = "_thumbs/";

/// Supported image MIME types.
const SUPPORTED_TYPES: &[&str] = &["image/jpeg", "image/png", "image/webp", "image/gif"];

/// Errors that can occur during gallery operations.
#[derive(Debug, Error)]
pub enum GalleryError {
    /// R2 storage error.
    #[error("R2 error: {0}")]
    R2(#[from] R2Error),

    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Image processing failed.
    #[error("image processing error: {0}")]
    ImageProcessing(String),

    /// Unsupported image format.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    /// File exceeds maximum size.
    #[error("file too large (max {MAX_IMAGE_SIZE} bytes)")]
    FileTooLarge,

    /// R2 gallery not configured.
    #[error("R2 gallery storage is not configured")]
    NotConfigured,

    /// Repository error.
    #[error("repository error: {0}")]
    Repository(#[from] crate::db::RepositoryError),
}

/// Parameters for uploading an image.
pub struct UploadImageParams {
    pub filename: String,
    pub content_type: String,
    pub data: Bytes,
    pub folder_prefix: String,
}

/// Result of a successful image upload.
pub struct UploadResult {
    pub r2_key: String,
    pub width: u32,
    pub height: u32,
    pub file_size: usize,
}

/// Result of thumbnail generation.
struct ThumbnailResult {
    thumb_bytes: Vec<u8>,
    original_width: u32,
    original_height: u32,
}

/// Check whether a content type is a supported image format.
#[must_use]
pub fn is_supported_image_type(content_type: &str) -> bool {
    SUPPORTED_TYPES.contains(&content_type)
}

/// Compute the thumbnail R2 key for a given original key.
///
/// For `products/serum/photo.jpg`, returns `_thumbs/products/serum/photo.webp`.
#[must_use]
pub fn thumb_key_for(original_key: &str) -> String {
    let without_ext = original_key
        .rsplit_once('.')
        .map_or(original_key, |(base, _)| base);
    format!("{THUMB_PREFIX}{without_ext}.webp")
}

/// Upload an image to R2, generating a thumbnail.
///
/// # Errors
///
/// Returns error if the image is too large, unsupported, or R2 operations fail.
#[instrument(skip(r2, params), fields(
    filename = %params.filename,
    size = params.data.len(),
    folder = %params.folder_prefix,
))]
pub async fn upload_image(
    r2: &R2Client,
    params: UploadImageParams,
) -> Result<UploadResult, GalleryError> {
    if params.data.len() > MAX_IMAGE_SIZE {
        return Err(GalleryError::FileTooLarge);
    }

    if !is_supported_image_type(&params.content_type) {
        return Err(GalleryError::UnsupportedFormat(params.content_type));
    }

    let r2_key = format!("{}{}", params.folder_prefix, params.filename);
    let file_size = params.data.len();
    let data_clone = params.data.clone();

    // Generate thumbnail in a blocking task (CPU-bound image processing)
    let thumb_result = tokio::task::spawn_blocking(move || generate_thumbnail(&data_clone))
        .await
        .map_err(|e| GalleryError::ImageProcessing(e.to_string()))?
        .map_err(GalleryError::ImageProcessing)?;

    // Upload original
    r2.put_object(&r2_key, params.data, &params.content_type)
        .await?;

    // Upload thumbnail
    let thumb_r2_key = thumb_key_for(&r2_key);
    r2.put_object(
        &thumb_r2_key,
        Bytes::from(thumb_result.thumb_bytes),
        "image/webp",
    )
    .await?;

    info!(
        key = %r2_key,
        width = thumb_result.original_width,
        height = thumb_result.original_height,
        "Image uploaded with thumbnail"
    );

    Ok(UploadResult {
        r2_key,
        width: thumb_result.original_width,
        height: thumb_result.original_height,
        file_size,
    })
}

/// Delete an image and its thumbnail from R2, plus metadata from DB.
///
/// # Errors
///
/// Returns error if R2 or DB operations fail.
#[instrument(skip(r2, pool), fields(key = %r2_key))]
pub async fn delete_image(r2: &R2Client, pool: &PgPool, r2_key: &str) -> Result<(), GalleryError> {
    let thumb_key = thumb_key_for(r2_key);
    let keys = vec![r2_key.to_string(), thumb_key];
    r2.delete_objects(&keys).await?;

    // Best-effort DB cleanup
    if let Err(e) = gallery_db::delete_metadata(pool, r2_key).await {
        warn!("Failed to delete gallery metadata (continuing): {e}");
    }

    info!("Gallery image deleted");
    Ok(())
}

/// Bulk delete images and their thumbnails from R2, plus metadata from DB.
///
/// # Errors
///
/// Returns error if R2 or DB operations fail.
#[instrument(skip(r2, pool), fields(count = r2_keys.len()))]
pub async fn delete_images(
    r2: &R2Client,
    pool: &PgPool,
    r2_keys: &[String],
) -> Result<(), GalleryError> {
    let mut all_keys: Vec<String> = Vec::with_capacity(r2_keys.len() * 2);
    for key in r2_keys {
        all_keys.push(key.clone());
        all_keys.push(thumb_key_for(key));
    }
    r2.delete_objects(&all_keys).await?;

    if let Err(e) = gallery_db::delete_metadata_batch(pool, r2_keys).await {
        warn!("Failed to bulk delete gallery metadata (continuing): {e}");
    }

    info!("Gallery images bulk deleted");
    Ok(())
}

/// Move or rename an image (server-side copy + delete).
///
/// # Errors
///
/// Returns error if R2 copy/delete or DB update fails.
#[instrument(skip(r2, pool), fields(source = %source_key, dest = %dest_key))]
pub async fn move_image(
    r2: &R2Client,
    pool: &PgPool,
    source_key: &str,
    dest_key: &str,
) -> Result<(), GalleryError> {
    // Copy original
    r2.copy_object(source_key, dest_key).await?;

    // Copy thumbnail
    let thumb_source = thumb_key_for(source_key);
    let thumb_dest = thumb_key_for(dest_key);
    if let Err(e) = r2.copy_object(&thumb_source, &thumb_dest).await {
        warn!("Failed to copy thumbnail (continuing): {e}");
    }

    // Delete old keys
    let old_keys = vec![source_key.to_string(), thumb_source];
    r2.delete_objects(&old_keys).await?;

    // Update DB metadata key (if row exists)
    if let Err(e) = gallery_db::update_metadata_key(pool, source_key, dest_key).await {
        warn!("Failed to update metadata key (continuing): {e}");
    }

    info!("Gallery image moved/renamed");
    Ok(())
}

/// Generate a WebP thumbnail from image bytes.
fn generate_thumbnail(data: &[u8]) -> Result<ThumbnailResult, String> {
    let img = image::load_from_memory(data).map_err(|e| format!("failed to decode image: {e}"))?;

    let original_width = img.width();
    let original_height = img.height();

    let thumb = img.thumbnail(THUMB_MAX_DIMENSION, THUMB_MAX_DIMENSION);

    let mut buf = std::io::Cursor::new(Vec::new());
    thumb
        .write_to(&mut buf, ImageFormat::WebP)
        .map_err(|e| format!("failed to encode thumbnail: {e}"))?;

    debug!(
        original = format!("{original_width}x{original_height}"),
        thumb = format!("{}x{}", thumb.width(), thumb.height()),
        "Generated thumbnail"
    );

    Ok(ThumbnailResult {
        thumb_bytes: buf.into_inner(),
        original_width,
        original_height,
    })
}

/// Format bytes into a human-readable string.
#[must_use]
pub fn format_file_size(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = 1024 * KB;

    #[allow(clippy::cast_precision_loss)]
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

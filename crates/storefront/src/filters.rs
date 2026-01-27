//! Custom Askama template filters.

#![allow(clippy::unnecessary_wraps)]

use std::fmt::Display;
use std::sync::LazyLock;

use crate::image_manifest;

/// Base URL for images, read from `IMAGE_BASE_URL` env var at runtime.
/// Defaults to "/static/images/derived" for local development.
static IMAGE_BASE_URL: LazyLock<String> = LazyLock::new(|| {
    std::env::var("IMAGE_BASE_URL").unwrap_or_else(|_| "/static/images/derived".to_string())
});

/// Returns the current year.
///
/// Usage in templates: `{{ ""|current_year }}`
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn current_year(_value: impl Display, _env: &dyn askama::Values) -> askama::Result<i32> {
    use chrono::Datelike;
    Ok(chrono::Utc::now().year())
}

/// Returns the content hash for an image path.
///
/// The input should be the base path without extension, e.g., "lifestyle/DSC_1068".
///
/// Usage in templates: `{{ "lifestyle/DSC_1068"|image_hash }}`
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn image_hash(base_path: impl Display, _env: &dyn askama::Values) -> askama::Result<String> {
    let path = base_path.to_string();
    Ok(image_manifest::get_image_hash(&path).to_string())
}

/// Returns the maximum generated width for an image path.
///
/// The input should be the base path without extension, e.g., "lifestyle/DSC_1068".
/// Returns 0 for SVGs (resolution-independent) or if image not found.
///
/// Usage in templates: `{{ "lifestyle/DSC_1068"|image_max_width }}`
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn image_max_width(base_path: impl Display, _env: &dyn askama::Values) -> askama::Result<u32> {
    let path = base_path.to_string();
    Ok(image_manifest::get_image_max_width(&path))
}

/// Generates a srcset string for responsive images, only including sizes that exist.
///
/// Parameters: base_path, hash, format (avif/webp/jpg)
///
/// Usage in templates: `{{ base|image_srcset(hash, "avif") }}`
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn image_srcset(
    base_path: &str,
    _env: &dyn askama::Values,
    hash: &str,
    format: &str,
) -> askama::Result<String> {
    const SIZES: [u32; 5] = [320, 640, 1024, 1600, 2400];

    let max_width = image_manifest::get_image_max_width(base_path);

    // If max_width is 0 (SVG or not found), include all sizes as fallback
    let effective_max = if max_width == 0 { 2400 } else { max_width };

    let base_url = &*IMAGE_BASE_URL;
    let srcset: Vec<String> = SIZES
        .iter()
        .filter(|&&size| size <= effective_max)
        .map(|&size| format!("{base_url}/{base_path}.{hash}-{size}.{format} {size}w"))
        .collect();

    Ok(srcset.join(", "))
}

/// Returns the largest available size for an image, for use as the default src.
///
/// Usage in templates: `{{ base|image_default_size }}`
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn image_default_size(
    base_path: impl Display,
    _env: &dyn askama::Values,
) -> askama::Result<u32> {
    const SIZES: [u32; 5] = [320, 640, 1024, 1600, 2400];

    let base = base_path.to_string();
    let max_width = image_manifest::get_image_max_width(&base);

    // Find the largest size that exists, defaulting to 1024
    let effective_max = if max_width == 0 { 1024 } else { max_width };

    Ok(SIZES
        .iter()
        .rev()
        .find(|&&size| size <= effective_max)
        .copied()
        .unwrap_or(1024))
}

/// Converts an original image path to a derived path with hash and size.
///
/// Input: "/static/images/original/hero/hero-self-love.png"
/// Output: "/static/images/derived/hero/hero-self-love.{hash}-{size}.jpg"
///
/// Usage in templates: `{{ path|to_derived_image(1600) }}`
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn to_derived_image(
    original_path: &str,
    _env: &dyn askama::Values,
    size: u32,
) -> askama::Result<String> {
    // Extract base path and extension
    let without_prefix = original_path.trim_start_matches("/static/images/original/");

    // Find the extension - all raster formats are converted to jpg
    let base = without_prefix
        .rfind('.')
        .map_or(without_prefix, |dot_pos| &without_prefix[..dot_pos]);

    let hash = image_manifest::get_image_hash(base);
    let max_width = image_manifest::get_image_max_width(base);

    // Use the requested size or the max available size, whichever is smaller
    let effective_size = if max_width > 0 && size > max_width {
        max_width
    } else {
        size
    };

    let base_url = &*IMAGE_BASE_URL;
    Ok(format!("{base_url}/{base}.{hash}-{effective_size}.jpg"))
}

/// Returns the base URL for derived images.
///
/// Reads from `IMAGE_BASE_URL` env var at runtime, defaults to "/static/images/derived".
///
/// Usage in templates: `{{ ""|image_base_url }}`
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn image_base_url(_value: impl Display, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(IMAGE_BASE_URL.clone())
}

/// Returns the content hash for main.css.
///
/// The hash is computed at build time from the CSS file content.
///
/// Usage in templates: `{{ ""|css_hash }}`
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn css_hash(_value: impl Display, _env: &dyn askama::Values) -> askama::Result<&'static str> {
    Ok(env!("CSS_HASH"))
}

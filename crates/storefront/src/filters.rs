//! Custom Askama template filters.

#![allow(clippy::unnecessary_wraps)]

use std::fmt::Display;
use std::sync::LazyLock;

use regex::Regex;

use crate::image_manifest;

/// Base URL for images, read from `IMAGE_BASE_URL` env var at runtime.
/// Defaults to "/static/images/derived" for local development.
static IMAGE_BASE_URL: LazyLock<String> = LazyLock::new(|| {
    std::env::var("IMAGE_BASE_URL").unwrap_or_else(|_| "/static/images/derived".to_string())
});

/// Constructs an absolute URL for the site logo (for JSON-LD structured data).
///
/// Uses the `branding/Logo_Horizontal` SVG with its content hash.
/// If `IMAGE_BASE_URL` is a CDN (starts with http), uses it directly.
/// Otherwise, prepends the site `base_url` to make it absolute.
#[must_use]
pub fn get_logo_url(base_url: &str) -> String {
    let img_base = &*IMAGE_BASE_URL;
    let hash = image_manifest::get_image_hash("branding/Logo_Horizontal");

    let logo_path = format!("branding/Logo_Horizontal.{hash}.svg");

    if img_base.starts_with("http") {
        // CDN URL - already absolute
        format!("{img_base}/{logo_path}")
    } else {
        // Relative path - prepend base_url
        format!("{base_url}{img_base}/{logo_path}")
    }
}

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

// =============================================================================
// SEO Filters
// =============================================================================

/// Strip the leading currency symbol ($) from a price string.
///
/// Usage in templates: `{{ product.price|strip_currency }}`
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn strip_currency(value: impl Display, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(value.to_string().trim_start_matches('$').to_string())
}

/// Regex for stripping HTML tags.
static HTML_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[^>]+>").expect("Invalid HTML tag regex"));

/// Regex for collapsing multiple whitespace characters.
static WHITESPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+").expect("Invalid whitespace regex"));

/// Strip HTML tags from a string for use in meta descriptions.
///
/// Also collapses multiple whitespace characters into single spaces
/// and trims leading/trailing whitespace.
///
/// Usage in templates: `{{ product.description|striptags }}`
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn striptags(value: impl Display, _env: &dyn askama::Values) -> askama::Result<String> {
    let html = value.to_string();
    let without_tags = HTML_TAG_RE.replace_all(&html, "");
    let normalized = WHITESPACE_RE.replace_all(&without_tags, " ");
    Ok(normalized.trim().to_string())
}

/// Truncate a string to a maximum length, adding "..." if truncated.
///
/// Tries to break at word boundaries when possible.
///
/// Usage in templates: `{{ description|truncate(160) }}`
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn truncate(value: &str, _env: &dyn askama::Values, max_len: usize) -> askama::Result<String> {
    if value.len() <= max_len {
        return Ok(value.to_string());
    }

    // Reserve space for ellipsis
    let target_len = max_len.saturating_sub(3);
    if target_len == 0 {
        return Ok("...".to_string());
    }

    // Find the last space before target_len to break at word boundary
    let truncated: String = value.chars().take(target_len).collect();
    let break_point = truncated.rfind(' ').unwrap_or(target_len);

    let result: String = value.chars().take(break_point).collect();
    Ok(format!("{}...", result.trim_end()))
}

// =============================================================================
// Analytics Filters
// =============================================================================

/// Cloudflare Web Analytics beacon token, read from `CF_BEACON_TOKEN` env var.
/// Returns empty string if not set (development).
static CF_BEACON_TOKEN: LazyLock<String> =
    LazyLock::new(|| std::env::var("CF_BEACON_TOKEN").unwrap_or_default());

/// Returns the Cloudflare beacon token for Web Analytics.
///
/// Returns empty string in development (when `CF_BEACON_TOKEN` is not set).
///
/// Usage in templates: `{% let cf_token = ""|cf_beacon_token %}`
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn cf_beacon_token(_value: impl Display, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(CF_BEACON_TOKEN.clone())
}

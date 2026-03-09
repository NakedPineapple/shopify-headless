//! Custom Askama template filters.

// The askama::filter_fn macro generates code that triggers these lints.
// The generated code doesn't inherit doc comments or allow attributes from the source.
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::inline_always)]
#![allow(clippy::unused_self)]
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
/// Uses the `branding/Pineapple_Skin_Co_Logo_Horizontal` SVG with its content hash.
/// If `IMAGE_BASE_URL` is a CDN (starts with http), uses it directly.
/// Otherwise, prepends the site `base_url` to make it absolute.
#[must_use]
pub fn get_logo_url(base_url: &str) -> String {
    let img_base = &*IMAGE_BASE_URL;
    let hash = image_manifest::get_image_hash("branding/Pineapple_Skin_Co_Logo_Horizontal");

    let logo_path = format!("branding/Pineapple_Skin_Co_Logo_Horizontal.{hash}.svg");

    if img_base.starts_with("http") {
        // CDN URL - already absolute
        format!("{img_base}/{logo_path}")
    } else {
        // Relative path - prepend base_url
        format!("{base_url}{img_base}/{logo_path}")
    }
}

/// Extract the numeric ID from a Shopify GID.
///
/// Examples:
/// - `gid://shopify/Collection/123` -> `123`
/// - `gid://shopify/Product/456` -> `456`
/// - `123` -> `123` (already numeric)
///
/// Usage in templates: `{{ id|extract_id }}`
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn extract_id(gid: &str, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(gid.split('/').next_back().unwrap_or(gid).to_string())
}

/// Returns the current year.
///
/// Usage in templates: `{{ ""|current_year }}`
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
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
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
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
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
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
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
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

    // If max_width is 0 (SVG or not found), include all preset sizes as fallback
    if max_width == 0 {
        let base_url = &*IMAGE_BASE_URL;
        let srcset: Vec<String> = SIZES
            .iter()
            .map(|&size| format!("{base_url}/{base_path}.{hash}-{size}.{format} {size}w"))
            .collect();
        return Ok(srcset.join(", "));
    }

    // Build list of sizes: preset sizes up to max_width + max_width itself if not a preset
    let base_url = &*IMAGE_BASE_URL;
    let mut sizes_to_include: Vec<u32> = SIZES
        .iter()
        .filter(|&&size| size <= max_width)
        .copied()
        .collect();

    // Add max_width if it's not already a preset size
    if !SIZES.contains(&max_width) {
        sizes_to_include.push(max_width);
    }

    let srcset: Vec<String> = sizes_to_include
        .iter()
        .map(|&size| format!("{base_url}/{base_path}.{hash}-{size}.{format} {size}w"))
        .collect();

    Ok(srcset.join(", "))
}

/// Returns the largest available size for an image, for use as the default src.
///
/// Usage in templates: `{{ base|image_default_size }}`
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn image_default_size(
    base_path: impl Display,
    _env: &dyn askama::Values,
) -> askama::Result<u32> {
    let base = base_path.to_string();
    let max_width = image_manifest::get_image_max_width(&base);

    // If max_width is 0 (SVG or not found), default to 1024
    // Otherwise return the actual max_width (which may be a non-preset size)
    Ok(if max_width == 0 { 1024 } else { max_width })
}

/// Converts an original image path to a derived path with hash and size.
///
/// Input: "/static/images/original/hero/hero-self-love.png"
/// Output: "/static/images/derived/hero/hero-self-love.{hash}-{size}.jpg"
///
/// Usage in templates: `{{ path|to_derived_image(1600) }}`
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
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
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
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
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
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
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
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
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
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
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
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

/// Sentry public DSN for client-side error reporting, read from `SENTRY_DSN_PUBLIC` env var.
/// Returns empty string if not set (development).
static SENTRY_DSN_PUBLIC: LazyLock<String> =
    LazyLock::new(|| std::env::var("SENTRY_DSN_PUBLIC").unwrap_or_default());

/// Sentry environment name, read from `SENTRY_ENVIRONMENT` env var.
/// Defaults to "production" if not set.
static SENTRY_ENVIRONMENT: LazyLock<String> = LazyLock::new(|| {
    std::env::var("SENTRY_ENVIRONMENT").unwrap_or_else(|_| "production".to_string())
});

/// Returns the Sentry public DSN for client-side error reporting.
///
/// Returns empty string in development (when `SENTRY_DSN_PUBLIC` is not set).
///
/// Usage in templates: `{% let sentry_dsn = ""|sentry_dsn_public %}`
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn sentry_dsn_public(
    _value: impl Display,
    _env: &dyn askama::Values,
) -> askama::Result<String> {
    Ok(SENTRY_DSN_PUBLIC.clone())
}

/// Returns the Sentry environment name.
///
/// Defaults to "production" when `SENTRY_ENVIRONMENT` is not set.
///
/// Usage in templates: `{% let sentry_env = ""|sentry_environment %}`
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn sentry_environment(
    _value: impl Display,
    _env: &dyn askama::Values,
) -> askama::Result<String> {
    Ok(SENTRY_ENVIRONMENT.clone())
}

/// Renders inline markdown to HTML.
///
/// Only renders inline elements (bold, italic, links, code) - no block elements.
/// Strips the wrapping `<p>` tag for seamless inline use.
///
/// Usage in templates: `{{ message|markdown }}`
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn markdown(value: impl Display, _env: &dyn askama::Values) -> askama::Result<String> {
    use comrak::{Options, markdown_to_html};

    let input = value.to_string();
    let html = markdown_to_html(&input, &Options::default());

    // Strip wrapping <p> tags for inline use
    let trimmed = html.trim();
    let result = trimmed
        .strip_prefix("<p>")
        .and_then(|s| s.strip_suffix("</p>"))
        .unwrap_or(trimmed);

    Ok(result.to_string())
}

// =============================================================================
// HTML Sanitization
// =============================================================================

/// Sanitize HTML content, allowing only safe tags and attributes.
///
/// This filter should be used on any HTML content from external sources
/// (Shopify product descriptions, blog content, etc.) before marking it
/// as safe in templates.
///
/// Allowed tags: p, br, strong, em, b, i, u, ul, ol, li, h1-h6, a, img, span, div,
///               table, thead, tbody, tr, th, td, blockquote, pre, code, hr
///
/// Links automatically get `rel="noopener noreferrer"` for security.
///
/// Usage in templates: `{{ product.description|sanitize_html|safe }}`
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn sanitize_html(html: impl Display, _env: &dyn askama::Values) -> askama::Result<String> {
    use std::collections::HashSet;

    // Define allowed tags for product/content HTML
    let allowed_tags: HashSet<&str> = [
        // Text formatting
        "p",
        "br",
        "strong",
        "em",
        "b",
        "i",
        "u",
        "s",
        "mark",
        "small",
        "sub",
        "sup",
        // Lists
        "ul",
        "ol",
        "li",
        // Headings
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        // Links and media
        "a",
        "img",
        // Containers
        "span",
        "div",
        "blockquote",
        "pre",
        "code",
        "hr",
        // Tables
        "table",
        "thead",
        "tbody",
        "tfoot",
        "tr",
        "th",
        "td",
        "caption",
    ]
    .into_iter()
    .collect();

    let sanitized = ammonia::Builder::default()
        .tags(allowed_tags)
        .link_rel(Some("noopener noreferrer"))
        .clean(&html.to_string())
        .to_string();

    Ok(sanitized)
}

// =============================================================================
// Shopify CDN Image Filters
// =============================================================================

/// Generates a `/cdn-cgi/image/` URL for a Shopify product image at a specific width.
///
/// Transforms a Shopify CDN URL into a Cloudflare Image Resizing URL.
/// In production, Cloudflare intercepts these URLs and applies transforms (AVIF/WebP).
/// In local dev, a fallback handler serves the original image.
///
/// Input:  `https://cdn.shopify.com/s/files/1/0123/image.jpg`, 800
/// Output: `/cdn-cgi/image/w=800,q=80,f=auto/images/shopify/s/files/1/0123/image.jpg`
///
/// Usage in templates: `{{ image.url|shopify_cdn_url(800) }}`
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn shopify_cdn_url(
    url: impl Display,
    _env: &dyn askama::Values,
    width: u32,
) -> askama::Result<String> {
    let url_str = url.to_string();

    // Extract the path from the Shopify CDN URL
    // Input: https://cdn.shopify.com/s/files/1/0123/image.jpg
    // We want: s/files/1/0123/image.jpg
    let path = url_str
        .strip_prefix("https://cdn.shopify.com/")
        .or_else(|| url_str.strip_prefix("http://cdn.shopify.com/"))
        .unwrap_or(&url_str);

    // Build the cdn-cgi URL with transform parameters
    // w=width, q=80 (quality), f=auto (format negotiation for AVIF/WebP)
    Ok(format!(
        "/cdn-cgi/image/w={width},q=80,f=auto/images/shopify/{path}"
    ))
}

/// Generates a srcset string with `/cdn-cgi/image/` URLs at multiple sizes.
///
/// Sizes: 320, 640, 1024, 1600, 2400
///
/// Input:  `https://cdn.shopify.com/s/files/1/0123/image.jpg`
/// Output: `/cdn-cgi/image/w=320,q=80,f=auto/images/shopify/... 320w, ...`
///
/// Usage in templates: `{{ image.url|shopify_srcset }}`
///
/// # Errors
///
/// This filter is infallible, however Askama requires filters return `askama::Result`.
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn shopify_srcset(url: impl Display, _env: &dyn askama::Values) -> askama::Result<String> {
    const SIZES: [u32; 5] = [320, 640, 1024, 1600, 2400];

    let url_str = url.to_string();

    // Extract the path from the Shopify CDN URL
    let path = url_str
        .strip_prefix("https://cdn.shopify.com/")
        .or_else(|| url_str.strip_prefix("http://cdn.shopify.com/"))
        .unwrap_or(&url_str);

    let srcset: Vec<String> = SIZES
        .iter()
        .map(|&size| format!("/cdn-cgi/image/w={size},q=80,f=auto/images/shopify/{path} {size}w"))
        .collect();

    Ok(srcset.join(", "))
}

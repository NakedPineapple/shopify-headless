//! Custom Askama template filters.

#![allow(clippy::unnecessary_wraps)]

use std::fmt::Display;

use crate::image_manifest;

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

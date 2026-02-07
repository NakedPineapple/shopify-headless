//! Authentication route utilities.

/// Validate a redirect URL is safe (prevents open redirects).
///
/// Only allows relative paths starting with `/` (not protocol-relative `//`).
#[must_use]
pub fn validate_redirect_url(url: &str) -> Option<&str> {
    if url.starts_with('/') && !url.starts_with("//") {
        Some(url)
    } else {
        None
    }
}

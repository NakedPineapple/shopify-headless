//! Shopify image proxy routes.
//!
//! Provides two routes for serving Shopify product images:
//!
//! 1. `GET /images/shopify/*path` - Origin route for Cloudflare to fetch images
//! 2. `GET /cdn-cgi/image/*rest` - Fallback for local dev (no Cloudflare)
//!
//! In production, Cloudflare intercepts `/cdn-cgi/image/` URLs, applies transforms
//! (AVIF/WebP conversion, resizing), and fetches the original from our origin at
//! `/images/shopify/`. In local dev without Cloudflare, the fallback handler serves
//! the original image directly (transforms ignored).

use axum::{
    extract::Path,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};

use crate::error::AppError;

/// Shared proxy logic - fetches image from Shopify CDN and streams the response.
async fn proxy_to_shopify(shopify_path: &str) -> Result<Response, AppError> {
    let url = format!("https://cdn.shopify.com/{shopify_path}");

    let response = reqwest::get(&url).await.map_err(|e| {
        tracing::warn!(url = %url, error = %e, "Failed to fetch Shopify image");
        AppError::NotFound("Image not found".to_string())
    })?;

    // Return 404 for missing images (don't expose Shopify error details)
    if !response.status().is_success() {
        return Err(AppError::NotFound("Image not found".to_string()));
    }

    // Extract content type from Shopify response
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    // Stream the response body
    let bytes = response.bytes().await.map_err(|e| {
        tracing::warn!(url = %url, error = %e, "Failed to read Shopify image body");
        AppError::Internal("Failed to read image".to_string())
    })?;

    // Build response with cache headers
    let mut headers = HeaderMap::new();

    // Content-Type from Shopify
    if let Ok(ct) = HeaderValue::from_str(&content_type) {
        headers.insert(header::CONTENT_TYPE, ct);
    }

    // Cache headers for edge caching (1 year, immutable)
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );

    Ok((StatusCode::OK, headers, bytes).into_response())
}

/// Route: GET /images/shopify/*path
///
/// Used by Cloudflare to fetch origin images for transformation.
/// In production, Cloudflare intercepts `/cdn-cgi/image/` requests, extracts
/// transform parameters, and fetches the original image from this endpoint.
///
/// # Errors
///
/// Returns `AppError::NotFound` if the image doesn't exist on Shopify CDN.
/// Returns `AppError::Internal` if there's a network error fetching the image.
pub async fn proxy_shopify_image(Path(path): Path<String>) -> Result<Response, AppError> {
    proxy_to_shopify(&path).await
}

/// Route: GET /cdn-cgi/image/*rest
///
/// Fallback handler for local development when Cloudflare isn't proxying.
/// Parses out the image path and serves the original (ignores transform params).
///
/// Input: `/cdn-cgi/image/w=800,q=80,f=auto/images/shopify/s/files/...`
/// Extracts: `s/files/...` and proxies from Shopify CDN.
///
/// # Errors
///
/// Returns `AppError::BadRequest` if the URL doesn't contain a valid Shopify path.
/// Returns `AppError::NotFound` if the image doesn't exist on Shopify CDN.
pub async fn cdn_cgi_fallback(Path(rest): Path<String>) -> Result<Response, AppError> {
    let shopify_path = extract_shopify_path(&rest)?;
    proxy_to_shopify(&shopify_path).await
}

/// Extracts the Shopify path from a cdn-cgi URL rest parameter.
///
/// Input: `w=800,q=80,f=auto/images/shopify/s/files/1/xxx/image.jpg`
/// Output: `s/files/1/xxx/image.jpg`
fn extract_shopify_path(rest: &str) -> Result<String, AppError> {
    // Find "/images/shopify/" marker in the path
    const MARKER: &str = "/images/shopify/";

    // The rest parameter may or may not have a leading slash depending on how
    // the URL was constructed. Handle both cases.
    let search_str = if rest.starts_with('/') {
        rest.to_string()
    } else {
        format!("/{rest}")
    };

    if let Some(idx) = search_str.find(MARKER) {
        let after_marker = &search_str[idx + MARKER.len()..];
        if after_marker.is_empty() {
            return Err(AppError::BadRequest("Invalid image path".to_string()));
        }
        Ok(after_marker.to_string())
    } else {
        Err(AppError::BadRequest(
            "Invalid cdn-cgi image path".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_shopify_path() {
        // Standard case with transform params
        let result =
            extract_shopify_path("w=800,q=80,f=auto/images/shopify/s/files/1/0123/image.jpg");
        assert_eq!(
            result.expect("should parse standard case"),
            "s/files/1/0123/image.jpg"
        );

        // With leading slash
        let result =
            extract_shopify_path("/w=800,q=80,f=auto/images/shopify/s/files/1/0123/image.jpg");
        assert_eq!(
            result.expect("should parse with leading slash"),
            "s/files/1/0123/image.jpg"
        );

        // No transform params (just the marker)
        let result = extract_shopify_path("/images/shopify/s/files/1/0123/image.jpg");
        assert_eq!(
            result.expect("should parse without transform params"),
            "s/files/1/0123/image.jpg"
        );

        // Invalid - missing marker
        let result = extract_shopify_path("w=800,q=80,f=auto/some/other/path.jpg");
        assert!(result.is_err());

        // Invalid - empty after marker
        let result = extract_shopify_path("/images/shopify/");
        assert!(result.is_err());
    }
}

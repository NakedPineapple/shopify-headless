//! Web app manifest route handler.

use axum::{
    http::header,
    response::{IntoResponse, Response},
};

use crate::image_manifest::get_image_hash;

/// Base URL for images from environment.
fn image_base_url() -> String {
    std::env::var("IMAGE_BASE_URL").unwrap_or_else(|_| "/static/images/derived".to_string())
}

/// Serve the web app manifest with hashed icon URLs.
pub async fn webmanifest() -> Response {
    let base = image_base_url();
    let hash_192 = get_image_hash("favicon/android-chrome-192x192");
    let hash_512 = get_image_hash("favicon/android-chrome-512x512");

    let manifest = serde_json::json!({
        "name": "Naked Pineapple",
        "short_name": "NP",
        "icons": [
            {
                "src": format!("{base}/favicon/android-chrome-192x192.{hash_192}.png"),
                "sizes": "192x192",
                "type": "image/png"
            },
            {
                "src": format!("{base}/favicon/android-chrome-512x512.{hash_512}.png"),
                "sizes": "512x512",
                "type": "image/png"
            }
        ],
        "theme_color": "#d63a2f",
        "background_color": "#fffbf7",
        "display": "standalone"
    });

    (
        [(header::CONTENT_TYPE, "application/manifest+json")],
        manifest.to_string(),
    )
        .into_response()
}

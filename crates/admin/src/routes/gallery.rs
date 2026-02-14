//! Image gallery route handlers.
//!
//! Browse, upload, delete, organize, and preview original images in R2.

use askama::Template;
use axum::{
    Json, Router,
    extract::{Multipart, Path, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};

use crate::{
    db::gallery as gallery_db, filters, middleware::auth::RequireAdminAuth, services::gallery,
    state::AppState,
};

use super::dashboard::AdminUserView;

// =============================================================================
// View types
// =============================================================================

/// Folder entry for templates.
struct FolderView {
    name: String,
    path: String,
}

/// Image grid item for templates.
struct ImageGridView {
    key: String,
    filename: String,
    thumb_url: String,
    file_size_display: String,
    last_modified: chrono::DateTime<chrono::Utc>,
    has_metadata: bool,
}

/// Breadcrumb segment.
struct BreadcrumbSegment {
    name: String,
    path: String,
}

/// Image detail for lightbox / metadata editing.
struct ImageDetailView {
    key: String,
    filename: String,
    content_type: String,
    file_size_display: String,
    last_modified: chrono::DateTime<chrono::Utc>,
    original_url: String,
    alt_text: String,
    description: String,
    custom_metadata: serde_json::Value,
}

// =============================================================================
// Templates
// =============================================================================

#[derive(Template)]
#[template(path = "gallery/index.html")]
struct GalleryIndexTemplate {
    admin_user: AdminUserView,
    current_path: String,
    prefix: String,
    breadcrumbs: Vec<BreadcrumbSegment>,
    folders: Vec<FolderView>,
    images: Vec<ImageGridView>,
    r2_configured: bool,
}

#[derive(Template)]
#[template(path = "gallery/partials/image_detail.html")]
struct ImageDetailTemplate {
    image: ImageDetailView,
}

#[derive(Template)]
#[template(path = "gallery/partials/folder_content.html")]
struct FolderContentPartialTemplate {
    prefix: String,
    folders: Vec<FolderView>,
    images: Vec<ImageGridView>,
}

// =============================================================================
// Router
// =============================================================================

/// Build gallery routes.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/gallery", get(index))
        .route("/gallery/folder/{*path}", get(folder_view))
        .route("/gallery/upload", post(upload))
        .route("/gallery/folder", post(create_folder))
        .route("/gallery/delete", post(delete_image))
        .route("/gallery/bulk-delete", post(bulk_delete))
        .route("/gallery/move", post(move_image))
        .route("/gallery/rename", post(rename_image))
        .route("/gallery/image/{*path}", get(image_detail))
        .route("/gallery/image/{*path}/metadata", post(update_metadata))
        .route("/gallery/serve/thumb/{*path}", get(serve_thumbnail))
        .route("/gallery/serve/original/{*path}", get(serve_original))
}

// =============================================================================
// Handlers
// =============================================================================

/// Gallery index (root folder).
#[instrument(skip(user, state, headers), fields(admin_id = %user.id.as_i32()))]
async fn index(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    render_folder(&user, &state, "", &headers).await
}

/// Gallery subfolder view.
#[instrument(skip(user, state, headers), fields(admin_id = %user.id.as_i32(), path = %path))]
async fn folder_view(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<String>,
) -> impl IntoResponse {
    let prefix = ensure_trailing_slash(&path);
    render_folder(&user, &state, &prefix, &headers).await
}

/// Upload images (multipart, multi-file).
#[instrument(skip(user, state, multipart), fields(admin_id = %user.id.as_i32()))]
async fn upload(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let Some(r2) = state.r2_gallery() else {
        return Json(serde_json::json!({"error": "Gallery storage not configured"}))
            .into_response();
    };

    let mut folder_prefix = String::new();
    let mut results: Vec<UploadResultView> = Vec::new();
    let mut files: Vec<(String, String, Bytes)> = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();

        match name.as_str() {
            "folder" => {
                if let Ok(text) = field.text().await {
                    folder_prefix = text;
                }
            }
            "files" => {
                let filename = field.file_name().unwrap_or("image").to_string();
                let content_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                match field.bytes().await {
                    Ok(bytes) => files.push((filename, content_type, bytes)),
                    Err(e) => {
                        error!("Failed to read uploaded file: {e}");
                    }
                }
            }
            _ => {}
        }
    }

    for (filename, content_type, data) in files {
        let params = gallery::UploadImageParams {
            filename: filename.clone(),
            content_type,
            data,
            folder_prefix: folder_prefix.clone(),
        };

        match gallery::upload_image(r2, params).await {
            Ok(result) => {
                info!(key = %result.r2_key, "Gallery image uploaded");
                results.push(UploadResultView {
                    key: result.r2_key.clone(),
                    thumb_url: format!(
                        "/gallery/serve/thumb/{}",
                        urlencoding::encode(&result.r2_key)
                    ),
                    filename,
                    width: result.width,
                    height: result.height,
                });
            }
            Err(e) => {
                error!(filename = %filename, "Gallery upload failed: {e}");
            }
        }
    }

    Json(results).into_response()
}

/// Create a folder marker in R2.
#[instrument(skip(user, state), fields(admin_id = %user.id.as_i32()))]
async fn create_folder(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
    axum::Form(form): axum::Form<CreateFolderForm>,
) -> impl IntoResponse {
    let Some(r2) = state.r2_gallery() else {
        return Redirect::to("/gallery").into_response();
    };

    let folder_key = format!("{}{}/", form.parent_prefix, form.folder_name);
    let marker_key = format!("{folder_key}.folder");

    if let Err(e) = r2
        .put_object(&marker_key, Bytes::new(), "application/x-directory")
        .await
    {
        error!("Failed to create folder marker: {e}");
    } else {
        info!(folder = %folder_key, "Gallery folder created");
    }

    let redirect_path = if form.parent_prefix.is_empty() {
        "/gallery".to_string()
    } else {
        format!("/gallery/folder/{}", form.parent_prefix)
    };
    Redirect::to(&redirect_path).into_response()
}

/// Delete a single image.
#[instrument(skip(user, state), fields(admin_id = %user.id.as_i32()))]
async fn delete_image(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
    axum::Form(form): axum::Form<DeleteForm>,
) -> impl IntoResponse {
    let Some(r2) = state.r2_gallery() else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "Gallery not configured",
        )
            .into_response();
    };

    match gallery::delete_image(r2, state.pool(), &form.key).await {
        Ok(()) => {
            info!(key = %form.key, "Gallery image deleted");
            axum::http::StatusCode::OK.into_response()
        }
        Err(e) => {
            error!("Failed to delete gallery image: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Delete failed",
            )
                .into_response()
        }
    }
}

/// Bulk delete images.
#[instrument(skip(user, state), fields(admin_id = %user.id.as_i32()))]
async fn bulk_delete(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
    Json(form): Json<BulkDeleteForm>,
) -> impl IntoResponse {
    let Some(r2) = state.r2_gallery() else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "Gallery not configured",
        )
            .into_response();
    };

    match gallery::delete_images(r2, state.pool(), &form.keys).await {
        Ok(()) => {
            info!(count = form.keys.len(), "Gallery images bulk deleted");
            axum::http::StatusCode::OK.into_response()
        }
        Err(e) => {
            error!("Bulk delete failed: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Bulk delete failed",
            )
                .into_response()
        }
    }
}

/// Move an image to a different folder.
#[instrument(skip(user, state), fields(admin_id = %user.id.as_i32()))]
async fn move_image(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
    Json(form): Json<MoveForm>,
) -> impl IntoResponse {
    let Some(r2) = state.r2_gallery() else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "Gallery not configured",
        )
            .into_response();
    };

    let filename = extract_filename(&form.source_key);
    let dest_key = format!("{}{filename}", form.dest_folder);

    match gallery::move_image(r2, state.pool(), &form.source_key, &dest_key).await {
        Ok(()) => {
            info!(
                source = %form.source_key,
                dest = %dest_key,
                "Gallery image moved"
            );
            Json(serde_json::json!({"new_key": dest_key})).into_response()
        }
        Err(e) => {
            error!("Move failed: {e}");
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Move failed").into_response()
        }
    }
}

/// Rename an image (same folder, different filename).
#[instrument(skip(user, state), fields(admin_id = %user.id.as_i32()))]
async fn rename_image(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
    Json(form): Json<RenameForm>,
) -> impl IntoResponse {
    let Some(r2) = state.r2_gallery() else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "Gallery not configured",
        )
            .into_response();
    };

    let folder = extract_folder(&form.source_key);
    let dest_key = format!("{folder}{}", form.new_filename);

    match gallery::move_image(r2, state.pool(), &form.source_key, &dest_key).await {
        Ok(()) => {
            info!(
                source = %form.source_key,
                dest = %dest_key,
                "Gallery image renamed"
            );
            Json(serde_json::json!({"new_key": dest_key})).into_response()
        }
        Err(e) => {
            error!("Rename failed: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Rename failed",
            )
                .into_response()
        }
    }
}

/// Image detail (HTMX partial for lightbox + metadata).
#[instrument(skip(user, state), fields(admin_id = %user.id.as_i32(), path = %path))]
async fn image_detail(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    let Some(r2) = state.r2_gallery() else {
        return Html("Gallery not configured".to_string()).into_response();
    };

    let r2_key = path;

    // Fetch R2 metadata + DB metadata in parallel
    let meta_future = r2.head_object(&r2_key);
    let db_future = gallery_db::get_metadata(state.pool(), &r2_key);
    let (meta_result, db_result) = tokio::join!(meta_future, db_future);

    let meta = match meta_result {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to head object: {e}");
            return Html("Image not found".to_string()).into_response();
        }
    };

    let db_meta = db_result.ok().flatten();

    let detail = ImageDetailView {
        filename: extract_filename(&r2_key),
        content_type: meta.content_type,
        file_size_display: gallery::format_file_size(meta.content_length),
        last_modified: meta.last_modified,
        original_url: format!("/gallery/serve/original/{}", urlencoding::encode(&r2_key)),
        alt_text: db_meta
            .as_ref()
            .and_then(|m| m.alt_text.clone())
            .unwrap_or_default(),
        description: db_meta
            .as_ref()
            .and_then(|m| m.description.clone())
            .unwrap_or_default(),
        custom_metadata: db_meta
            .as_ref()
            .map_or_else(|| serde_json::json!({}), |m| m.custom_metadata.clone()),
        key: r2_key,
    };

    let template = ImageDetailTemplate { image: detail };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// Update image metadata (save to DB).
#[instrument(skip(user, state), fields(admin_id = %user.id.as_i32(), path = %path))]
async fn update_metadata(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
    Path(path): Path<String>,
    axum::Form(form): axum::Form<MetadataForm>,
) -> impl IntoResponse {
    let r2_key = path;

    let custom_metadata =
        serde_json::from_str(&form.custom_metadata_json).unwrap_or_else(|_| serde_json::json!({}));

    let params = gallery_db::UpsertMetadataParams {
        r2_key: r2_key.clone(),
        alt_text: if form.alt_text.is_empty() {
            None
        } else {
            Some(form.alt_text)
        },
        description: if form.description.is_empty() {
            None
        } else {
            Some(form.description)
        },
        custom_metadata,
        updated_by: user.id.as_i32(),
    };

    match gallery_db::upsert_metadata(state.pool(), &params).await {
        Ok(()) => {
            info!(key = %r2_key, "Gallery metadata updated");
            Html(r#"<div class="text-sm text-emerald-400">Saved!</div>"#.to_string())
                .into_response()
        }
        Err(e) => {
            error!("Failed to save metadata: {e}");
            Html(r#"<div class="text-sm text-red-400">Save failed</div>"#.to_string())
                .into_response()
        }
    }
}

/// Serve a thumbnail from R2.
#[instrument(skip(_user, state), fields(path = %path))]
async fn serve_thumbnail(
    RequireAdminAuth(_user): RequireAdminAuth,
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    let Some(r2) = state.r2_gallery() else {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    };

    let thumb_key = gallery::thumb_key_for(&path);

    match r2.get_object(&thumb_key).await {
        Ok(bytes) => {
            let headers = [
                (axum::http::header::CONTENT_TYPE, "image/webp".to_string()),
                (
                    axum::http::header::CACHE_CONTROL,
                    "public, max-age=86400".to_string(),
                ),
            ];
            (headers, bytes).into_response()
        }
        Err(_) => {
            // Thumbnail might not exist yet — try serving original as fallback
            r2.get_object(&path).await.map_or_else(
                |_| axum::http::StatusCode::NOT_FOUND.into_response(),
                |bytes| {
                    let headers = [(
                        axum::http::header::CACHE_CONTROL,
                        "public, max-age=3600".to_string(),
                    )];
                    (headers, bytes).into_response()
                },
            )
        }
    }
}

/// Serve the original image from R2.
#[instrument(skip(_user, state), fields(path = %path))]
async fn serve_original(
    RequireAdminAuth(_user): RequireAdminAuth,
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    let Some(r2) = state.r2_gallery() else {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    };

    let filename = extract_filename(&path);

    match r2.get_object(&path).await {
        Ok(bytes) => {
            let content_type = mime_from_extension(&filename);
            let headers = [
                (axum::http::header::CONTENT_TYPE, content_type),
                (
                    axum::http::header::CONTENT_DISPOSITION,
                    format!("inline; filename=\"{filename}\""),
                ),
                (
                    axum::http::header::CACHE_CONTROL,
                    "public, max-age=86400".to_string(),
                ),
            ];
            (headers, bytes).into_response()
        }
        Err(e) => {
            error!("Failed to serve original: {e}");
            axum::http::StatusCode::NOT_FOUND.into_response()
        }
    }
}

// =============================================================================
// Form types
// =============================================================================

#[derive(Debug, Deserialize)]
struct CreateFolderForm {
    parent_prefix: String,
    folder_name: String,
}

#[derive(Debug, Deserialize)]
struct DeleteForm {
    key: String,
}

#[derive(Debug, Deserialize)]
struct BulkDeleteForm {
    keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MoveForm {
    source_key: String,
    dest_folder: String,
}

#[derive(Debug, Deserialize)]
struct RenameForm {
    source_key: String,
    new_filename: String,
}

#[derive(Debug, Deserialize)]
struct MetadataForm {
    alt_text: String,
    description: String,
    custom_metadata_json: String,
}

#[derive(Serialize)]
struct UploadResultView {
    key: String,
    thumb_url: String,
    filename: String,
    width: u32,
    height: u32,
}

// =============================================================================
// Helpers
// =============================================================================

/// Render a folder listing.
///
/// When the `HX-Request` header is present, returns only the grid partial
/// (folders + images) for lazy-loading via HTMX.
async fn render_folder(
    user: &crate::models::session::CurrentAdmin,
    state: &AppState,
    prefix: &str,
    headers: &HeaderMap,
) -> axum::response::Response {
    let r2_configured = state.r2_gallery().is_some();
    let is_htmx = headers.contains_key("hx-request");

    let (folders, images) = if let Some(r2) = state.r2_gallery() {
        match r2.list_objects(prefix, "/").await {
            Ok((entries, prefixes)) => {
                // Build folder views — filter out _thumbs/ prefix
                let folders: Vec<FolderView> = prefixes
                    .into_iter()
                    .filter(|p| !p.starts_with("_thumbs/"))
                    .map(|p| {
                        let name = p.trim_end_matches('/').rsplit_once('/').map_or_else(
                            || p.trim_end_matches('/').to_string(),
                            |(_, name)| name.to_string(),
                        );
                        FolderView { name, path: p }
                    })
                    .collect();

                // Filter to image files only
                let image_entries: Vec<_> = entries
                    .into_iter()
                    .filter(|e| is_image_key(&e.key))
                    .collect();

                // Fetch metadata batch
                let keys: Vec<String> = image_entries.iter().map(|e| e.key.clone()).collect();
                let metadata_map = gallery_db::get_metadata_batch(state.pool(), &keys)
                    .await
                    .unwrap_or_default();

                let images: Vec<ImageGridView> = image_entries
                    .into_iter()
                    .map(|e| {
                        let has_metadata = metadata_map.contains_key(&e.key);
                        ImageGridView {
                            filename: extract_filename(&e.key),
                            thumb_url: format!(
                                "/gallery/serve/thumb/{}",
                                urlencoding::encode(&e.key)
                            ),
                            file_size_display: gallery::format_file_size(e.size),
                            last_modified: e.last_modified,
                            has_metadata,
                            key: e.key,
                        }
                    })
                    .collect();

                (folders, images)
            }
            Err(e) => {
                error!("Failed to list gallery objects: {e}");
                (Vec::new(), Vec::new())
            }
        }
    } else {
        (Vec::new(), Vec::new())
    };

    // HTMX request — return grid partial only (for lazy loading)
    if is_htmx {
        let partial = FolderContentPartialTemplate {
            prefix: prefix.to_string(),
            folders,
            images,
        };

        return Html(
            partial
                .render()
                .unwrap_or_else(|e| format!("Template error: {e}")),
        )
        .into_response();
    }

    let breadcrumbs = build_breadcrumbs(prefix);

    let template = GalleryIndexTemplate {
        admin_user: AdminUserView::from(user),
        current_path: "/gallery".to_string(),
        prefix: prefix.to_string(),
        breadcrumbs,
        folders,
        images,
        r2_configured,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// Build breadcrumb segments from a prefix path.
fn build_breadcrumbs(prefix: &str) -> Vec<BreadcrumbSegment> {
    let mut breadcrumbs = vec![BreadcrumbSegment {
        name: "Gallery".to_string(),
        path: "/gallery".to_string(),
    }];

    if prefix.is_empty() {
        return breadcrumbs;
    }

    let mut accumulated = String::new();
    for part in prefix.trim_end_matches('/').split('/') {
        if part.is_empty() {
            continue;
        }
        accumulated.push_str(part);
        accumulated.push('/');
        breadcrumbs.push(BreadcrumbSegment {
            name: part.to_string(),
            path: format!("/gallery/folder/{accumulated}"),
        });
    }

    breadcrumbs
}

/// Extract filename from an R2 key.
fn extract_filename(key: &str) -> String {
    key.rsplit_once('/')
        .map_or_else(|| key.to_string(), |(_, name)| name.to_string())
}

/// Extract folder prefix from an R2 key.
fn extract_folder(key: &str) -> String {
    key.rsplit_once('/')
        .map_or_else(String::new, |(folder, _)| format!("{folder}/"))
}

/// Check if an R2 key looks like an image file.
fn is_image_key(key: &str) -> bool {
    let path = std::path::Path::new(key);
    path.extension().is_some_and(|ext| {
        ext.eq_ignore_ascii_case("jpg")
            || ext.eq_ignore_ascii_case("jpeg")
            || ext.eq_ignore_ascii_case("png")
            || ext.eq_ignore_ascii_case("webp")
            || ext.eq_ignore_ascii_case("gif")
    })
}

/// Ensure a path ends with '/'.
fn ensure_trailing_slash(path: &str) -> String {
    if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{path}/")
    }
}

/// Guess MIME type from file extension.
fn mime_from_extension(filename: &str) -> String {
    let ext = std::path::Path::new(filename).extension();
    match ext {
        Some(e) if e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg") => {
            "image/jpeg".to_string()
        }
        Some(e) if e.eq_ignore_ascii_case("png") => "image/png".to_string(),
        Some(e) if e.eq_ignore_ascii_case("webp") => "image/webp".to_string(),
        Some(e) if e.eq_ignore_ascii_case("gif") => "image/gif".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

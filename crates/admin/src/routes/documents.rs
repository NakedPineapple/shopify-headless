//! Document management route handlers.
//!
//! Upload, view, download, and delete business documents for AI chat reference.

use askama::Template;
use axum::{
    Router,
    extract::{Multipart, Path, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use bytes::Bytes;
use tracing::{debug, error, info, instrument, warn};

use crate::{
    db::documents as doc_db,
    filters,
    middleware::auth::RequireAdminAuth,
    services::documents::{self, DocumentError, UploadParams},
    state::AppState,
};

use super::dashboard::AdminUserView;

// =============================================================================
// View types
// =============================================================================

/// Document view for templates.
struct DocumentView {
    id: i32,
    filename: String,
    content_type: String,
    file_size_display: String,
    description: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl DocumentView {
    fn from_row(row: doc_db::DocumentRow) -> Self {
        Self {
            id: row.id,
            filename: row.filename,
            content_type: row.content_type,
            file_size_display: format_file_size(row.file_size),
            description: row.description,
            created_at: row.created_at,
        }
    }
}

/// Format bytes into a human-readable string.
fn format_file_size(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = 1024 * KB;

    // File sizes are far below f64 mantissa limits, so precision loss is negligible.
    #[allow(clippy::cast_precision_loss)]
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

// =============================================================================
// Templates
// =============================================================================

#[derive(Template)]
#[template(path = "documents/index.html")]
struct DocumentsIndexTemplate {
    admin_user: AdminUserView,
    current_path: String,
    documents: Vec<DocumentView>,
}

#[derive(Template)]
#[template(path = "documents/new.html")]
struct DocumentNewTemplate {
    admin_user: AdminUserView,
    current_path: String,
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "documents/show.html")]
struct DocumentShowTemplate {
    admin_user: AdminUserView,
    current_path: String,
    document: DocumentView,
    chunk_count: i64,
}

// =============================================================================
// Router
// =============================================================================

/// Build document routes.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/documents", get(index))
        .route("/documents/new", get(new_document))
        .route("/documents", post(upload))
        .route("/documents/{id}", get(show))
        .route("/documents/{id}/download", get(download))
        .route("/documents/{id}/delete", post(delete))
}

// =============================================================================
// Handlers
// =============================================================================

/// List all documents.
#[instrument(skip(user, state), fields(admin_id = %user.id.as_i32()))]
async fn index(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
) -> impl IntoResponse {
    debug!("Loading documents list");

    let rows = doc_db::list_documents(state.pool(), 200)
        .await
        .unwrap_or_else(|e| {
            error!("Failed to list documents: {e}");
            vec![]
        });

    let documents = rows.into_iter().map(DocumentView::from_row).collect();

    let template = DocumentsIndexTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/documents".to_string(),
        documents,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
}

/// Show the upload form.
async fn new_document(RequireAdminAuth(user): RequireAdminAuth) -> impl IntoResponse {
    let template = DocumentNewTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/documents".to_string(),
        error: None,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
}

/// Upload a document (multipart form).
#[instrument(skip(user, state, multipart), fields(admin_id = %user.id.as_i32()))]
async fn upload(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // Check prerequisites
    let Some(r2) = state.r2() else {
        return render_upload_error(&user, "R2 storage is not configured");
    };
    let Some(embedding_client) = state.embedding() else {
        return render_upload_error(&user, "Embedding service is not configured");
    };

    // Extract fields from multipart form
    let mut file_data: Option<(String, String, Bytes)> = None;
    let mut description: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();

        match name.as_str() {
            "file" => {
                let filename = field.file_name().unwrap_or("document").to_string();
                let content_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();

                match field.bytes().await {
                    Ok(bytes) => {
                        if bytes.len() > documents::MAX_FILE_SIZE {
                            return render_upload_error(
                                &user,
                                &DocumentError::FileTooLarge.to_string(),
                            );
                        }
                        file_data = Some((filename, content_type, bytes));
                    }
                    Err(e) => {
                        error!("Failed to read uploaded file: {e}");
                        return render_upload_error(&user, "Failed to read uploaded file");
                    }
                }
            }
            "description" => {
                if let Ok(text) = field.text().await {
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        description = Some(trimmed);
                    }
                }
            }
            _ => {}
        }
    }

    let Some((filename, content_type, bytes)) = file_data else {
        return render_upload_error(&user, "No file was provided");
    };

    if !documents::is_supported_type(&content_type) {
        return render_upload_error(
            &user,
            "Unsupported file type. Accepted formats: PDF, TXT, MD.",
        );
    }

    let params = UploadParams {
        filename: filename.clone(),
        content_type,
        data: bytes,
        uploaded_by: user.id.as_i32(),
        description,
    };

    match documents::upload_document(state.pool(), r2, embedding_client, params).await {
        Ok(result) => {
            info!(
                document_id = result.document_id,
                filename = %filename,
                chunks = result.chunk_count,
                "Document uploaded and indexed"
            );
            Redirect::to(&format!("/documents/{}", result.document_id)).into_response()
        }
        Err(e) => {
            error!("Document upload failed: {e}");
            let msg = match &e {
                DocumentError::EmptyDocument => {
                    "No text could be extracted from this document.".to_string()
                }
                DocumentError::UnsupportedFormat(f) => {
                    format!("Unsupported file format: {f}")
                }
                DocumentError::ExtractionFailed(detail) => {
                    format!("Text extraction failed: {detail}")
                }
                DocumentError::FileTooLarge => e.to_string(),
                _ => format!("Upload failed: {e}"),
            };
            render_upload_error(&user, &msg)
        }
    }
}

/// Show document details.
#[instrument(skip(user, state), fields(admin_id = %user.id.as_i32(), document_id = id))]
async fn show(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let row = match doc_db::get_document(state.pool(), id).await {
        Ok(Some(d)) => d,
        Ok(None) => return Html("Document not found".to_string()).into_response(),
        Err(e) => {
            error!("Failed to fetch document: {e}");
            return Html("Failed to load document".to_string()).into_response();
        }
    };

    let chunk_count = doc_db::get_chunk_count(state.pool(), id).await.unwrap_or(0);

    let template = DocumentShowTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/documents".to_string(),
        document: DocumentView::from_row(row),
        chunk_count,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// Download the original document from R2.
#[instrument(skip(user, state), fields(admin_id = %user.id.as_i32(), document_id = id))]
async fn download(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let Some(r2) = state.r2() else {
        return Html("R2 storage is not configured".to_string()).into_response();
    };

    let row = match doc_db::get_document(state.pool(), id).await {
        Ok(Some(d)) => d,
        Ok(None) => return Html("Document not found".to_string()).into_response(),
        Err(e) => {
            error!("Failed to fetch document: {e}");
            return Html("Failed to load document".to_string()).into_response();
        }
    };

    match r2.get_object(&row.r2_key).await {
        Ok(bytes) => {
            let headers = [
                (axum::http::header::CONTENT_TYPE, row.content_type),
                (
                    axum::http::header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", row.filename),
                ),
            ];
            (headers, bytes).into_response()
        }
        Err(e) => {
            error!("Failed to download from R2: {e}");
            Html("Download failed".to_string()).into_response()
        }
    }
}

/// Delete a document (R2 object + database rows).
#[instrument(skip(user, state), fields(admin_id = %user.id.as_i32(), document_id = id))]
async fn delete(
    RequireAdminAuth(user): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let row = match doc_db::get_document(state.pool(), id).await {
        Ok(Some(d)) => d,
        Ok(None) => return Redirect::to("/documents").into_response(),
        Err(e) => {
            error!("Failed to fetch document for deletion: {e}");
            return Redirect::to("/documents").into_response();
        }
    };

    // Delete from R2 first (best-effort — if R2 fails we still clean DB)
    if let Some(r2) = state.r2()
        && let Err(e) = r2.delete_object(&row.r2_key).await
    {
        warn!("Failed to delete from R2 (continuing with DB cleanup): {e}");
    }

    if let Err(e) = doc_db::delete_document(state.pool(), id).await {
        error!("Failed to delete document from database: {e}");
    } else {
        info!(document_id = id, filename = %row.filename, "Document deleted");
    }

    Redirect::to("/documents").into_response()
}

// =============================================================================
// Helpers
// =============================================================================

/// Render the upload form with an error message.
fn render_upload_error(
    user: &crate::models::session::CurrentAdmin,
    message: &str,
) -> axum::response::Response {
    let template = DocumentNewTemplate {
        admin_user: AdminUserView::from(user),
        current_path: "/documents".to_string(),
        error: Some(message.to_string()),
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

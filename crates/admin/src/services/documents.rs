//! Document processing service: text extraction, chunking, and embedding.
//!
//! Handles the full pipeline from file upload to searchable embedding storage.

use bytes::Bytes;
use sqlx::PgPool;
use thiserror::Error;
use tracing::{debug, info, instrument};

use naked_pineapple_services::openai::{EmbeddingClient, EmbeddingError};

use crate::r2::{R2Client, R2Error};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during document processing.
#[derive(Debug, Error)]
pub enum DocumentError {
    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// R2 storage error.
    #[error("R2 error: {0}")]
    R2(#[from] R2Error),

    /// Embedding generation error.
    #[error("embedding error: {0}")]
    Embedding(#[from] EmbeddingError),

    /// Text extraction failed.
    #[error("text extraction failed: {0}")]
    ExtractionFailed(String),

    /// File format is not supported.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    /// Document contained no extractable text.
    #[error("document is empty after text extraction")]
    EmptyDocument,

    /// File exceeds the maximum allowed size.
    #[error("file too large (max {MAX_FILE_SIZE} bytes)")]
    FileTooLarge,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parameters for uploading a document.
pub struct UploadParams {
    pub filename: String,
    pub content_type: String,
    pub data: Bytes,
    pub uploaded_by: i32,
    pub description: Option<String>,
}

/// Result of a successful document upload.
pub struct UploadResult {
    pub document_id: i32,
    pub chunk_count: usize,
}

/// Upload, extract, chunk, embed, and store a document.
///
/// # Pipeline
///
/// 1. Extract text from file bytes
/// 2. Chunk text into ~500-token segments with overlap
/// 3. Generate embeddings via `OpenAI` batch API
/// 4. Insert document row, upload to R2, insert chunks — all in a transaction
///
/// # Errors
///
/// Returns error if any pipeline step fails. On failure the transaction
/// is rolled back and no partial data is persisted.
#[instrument(skip(pool, r2, embedding, params), fields(
    filename = %params.filename,
    size = params.data.len(),
))]
pub async fn upload_document(
    pool: &PgPool,
    r2: &R2Client,
    embedding: &EmbeddingClient,
    params: UploadParams,
) -> Result<UploadResult, DocumentError> {
    // Step 1: Extract text
    let text = extract_text(&params.data, &params.content_type)?;
    debug!(text_len = text.len(), "Extracted text from document");

    // Step 2: Chunk
    let chunks = chunk_text(&text);
    debug!(chunk_count = chunks.len(), "Chunked document text");

    if chunks.is_empty() {
        return Err(DocumentError::EmptyDocument);
    }

    // Step 3: Embed all chunks in one batch request
    let chunk_texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
    let embeddings = embedding.embed_batch(&chunk_texts).await?;
    debug!("Generated embeddings for all chunks");

    // Step 4: Persist everything in a transaction
    let mut tx = pool.begin().await?;

    // Insert document metadata (r2_key is a placeholder until we know the ID)
    let file_size = i64::try_from(params.data.len()).unwrap_or(i64::MAX);
    let doc_id: i32 = sqlx::query_scalar(
        r"
        INSERT INTO admin.documents (filename, content_type, file_size, r2_key, uploaded_by, description)
        VALUES ($1, $2, $3, '', $4, $5)
        RETURNING id
        ",
    )
    .bind(&params.filename)
    .bind(&params.content_type)
    .bind(file_size)
    .bind(params.uploaded_by)
    .bind(&params.description)
    .fetch_one(&mut *tx)
    .await?;

    // Upload to R2 with the real key
    let r2_key = format!("documents/{doc_id}/{}", params.filename);
    r2.put_object(&r2_key, params.data, &params.content_type)
        .await?;

    // Update the r2_key now that we know the document ID
    sqlx::query(r"UPDATE admin.documents SET r2_key = $1 WHERE id = $2")
        .bind(&r2_key)
        .bind(doc_id)
        .execute(&mut *tx)
        .await?;

    // Insert chunks with embeddings
    let chunk_count = chunks.len();
    for (idx, (chunk, emb)) in chunks.iter().zip(embeddings.iter()).enumerate() {
        let embedding_str = format_embedding(emb);
        let chunk_idx = i32::try_from(idx).unwrap_or(i32::MAX);
        let token_count = i32::try_from(chunk.token_count).unwrap_or(i32::MAX);

        sqlx::query(
            r"
            INSERT INTO admin.document_chunks (document_id, chunk_index, chunk_text, token_count, embedding)
            VALUES ($1, $2, $3, $4, $5::vector)
            ",
        )
        .bind(doc_id)
        .bind(chunk_idx)
        .bind(&chunk.text)
        .bind(token_count)
        .bind(&embedding_str)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    info!(
        document_id = doc_id,
        chunks = chunk_count,
        "Document uploaded and indexed"
    );
    Ok(UploadResult {
        document_id: doc_id,
        chunk_count,
    })
}

// ---------------------------------------------------------------------------
// Text extraction
// ---------------------------------------------------------------------------

/// Extract text from a file based on its content type.
fn extract_text(data: &Bytes, content_type: &str) -> Result<String, DocumentError> {
    match content_type {
        "application/pdf" => extract_pdf(data),
        "text/plain" | "text/markdown" => extract_utf8(data),
        other => Err(DocumentError::UnsupportedFormat(other.to_string())),
    }
}

/// Extract text from a PDF using `pdf-extract`.
fn extract_pdf(data: &Bytes) -> Result<String, DocumentError> {
    pdf_extract::extract_text_from_mem(data)
        .map_err(|e| DocumentError::ExtractionFailed(e.to_string()))
}

/// Interpret bytes as UTF-8 text.
fn extract_utf8(data: &Bytes) -> Result<String, DocumentError> {
    String::from_utf8(data.to_vec()).map_err(|e| DocumentError::ExtractionFailed(e.to_string()))
}

// ---------------------------------------------------------------------------
// Chunking
// ---------------------------------------------------------------------------

/// A text chunk with an estimated token count.
struct Chunk {
    text: String,
    token_count: usize,
}

/// Target number of tokens per chunk (~4 chars per token).
const TARGET_TOKENS: usize = 500;
/// Characters per token estimate.
const CHARS_PER_TOKEN: usize = 4;
/// Overlap in characters between consecutive chunks.
const OVERLAP_CHARS: usize = 50 * CHARS_PER_TOKEN; // ~50 tokens

/// Split text into chunks at paragraph boundaries.
///
/// Merges small paragraphs together until the target size is reached,
/// then starts a new chunk with a small overlap from the previous one.
fn chunk_text(text: &str) -> Vec<Chunk> {
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();

    let mut chunks = Vec::new();
    let mut current = String::new();

    for para in &paragraphs {
        let combined_len = current.len() + para.len() + 2; // +2 for \n\n separator

        if !current.is_empty() && combined_len > TARGET_TOKENS * CHARS_PER_TOKEN {
            // Finalize current chunk
            let token_count = current.len() / CHARS_PER_TOKEN;
            let overlap = tail_overlap(&current);
            chunks.push(Chunk {
                text: current,
                token_count,
            });
            current = overlap;
        }

        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(para);
    }

    // Don't forget the last chunk
    if !current.is_empty() {
        let token_count = current.len() / CHARS_PER_TOKEN;
        chunks.push(Chunk {
            text: current,
            token_count,
        });
    }

    chunks
}

/// Return the last `OVERLAP_CHARS` characters of a string for chunk overlap.
fn tail_overlap(text: &str) -> String {
    if text.len() <= OVERLAP_CHARS {
        return text.to_string();
    }
    // Start at a char boundary
    let start = text.len() - OVERLAP_CHARS;
    let start = text.ceil_char_boundary(start);
    text[start..].to_string()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format an embedding vector as a pgvector literal string.
fn format_embedding(embedding: &[f32]) -> String {
    let values: Vec<String> = embedding.iter().map(ToString::to_string).collect();
    format!("[{}]", values.join(","))
}

/// Content types we accept for document upload.
pub const SUPPORTED_TYPES: &[&str] = &["application/pdf", "text/plain", "text/markdown"];

/// Maximum file size in bytes (10 MB).
pub const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Check whether a content type is supported.
#[must_use]
pub fn is_supported_type(content_type: &str) -> bool {
    SUPPORTED_TYPES.contains(&content_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text_empty() {
        let chunks = chunk_text("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_text_single_paragraph() {
        let text = "Hello world, this is a test.";
        let chunks = chunk_text(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks.first().map(|c| c.text.as_str()), Some(text));
    }

    #[test]
    fn test_chunk_text_splits_large_text() {
        // Create text that's larger than TARGET_TOKENS * CHARS_PER_TOKEN
        let para = "A".repeat(TARGET_TOKENS * CHARS_PER_TOKEN + 100);
        let text = format!("{para}\n\n{para}");
        let chunks = chunk_text(&text);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_format_embedding() {
        let emb = vec![0.1, 0.2, 0.3];
        let s = format_embedding(&emb);
        assert_eq!(s, "[0.1,0.2,0.3]");
    }

    #[test]
    fn test_is_supported_type() {
        assert!(is_supported_type("application/pdf"));
        assert!(is_supported_type("text/plain"));
        assert!(is_supported_type("text/markdown"));
        assert!(!is_supported_type("image/png"));
    }

    #[test]
    fn test_extract_utf8() {
        let data = Bytes::from("hello world");
        let result = extract_utf8(&data).expect("should succeed");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_extract_utf8_invalid() {
        let data = Bytes::from(vec![0xFF, 0xFE]);
        assert!(extract_utf8(&data).is_err());
    }

    #[test]
    fn test_tail_overlap_short_text() {
        let text = "short";
        assert_eq!(tail_overlap(text), "short");
    }
}
